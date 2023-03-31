use bevy_app::{App, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::event::{EventReader, EventWriter};
use bevy_ecs::query::{Changed, With};
use bevy_ecs::schedule::IntoSystemConfig;
use bevy_ecs::system::{Commands, Query, Res};
use bevy_time::{Timer, TimerMode};

use yewoh::protocol::EquipmentSlot;
use yewoh_server::world::entity::{AttackTarget, Character, Flags, Graphic, MapPosition, Quantity, Stats};
use yewoh_server::world::events::AttackRequestedEvent;
use yewoh_server::world::hierarchy::DespawnRecursiveExt;
use yewoh_server::world::net::{NetEntity, NetEntityAllocator, Possessing};

use crate::activities::{CurrentActivity, progress_current_activity};
use crate::characters::{Alive, CharacterDied, CorpseSpawned, DamageDealt, MeleeWeapon, Unarmed};

pub const CORPSE_GRAPHIC_ID: u16 = 0x2006;

pub fn handle_attack_requests(
    mut commands: Commands,
    mut requests: EventReader<AttackRequestedEvent>,
    clients: Query<&Possessing>,
) {
    for request in &mut requests {
        let possessing = match clients.get(request.client_entity) {
            Ok(x) => x,
            _ => continue,
        };

        commands.entity(possessing.entity).insert(AttackTarget {
            target: request.target,
        });
    }
}

pub fn update_weapon_stats(
    mut commands: Commands,
    mut characters: Query<(Entity, &Character, Option<&Unarmed>), (Changed<Character>, Changed<Unarmed>)>,
    weapons: Query<&MeleeWeapon>,
) {
    for (entity, character, unarmed) in &mut characters {
        let weapon = character.equipment.iter()
            .filter(|e| e.slot == EquipmentSlot::MainHand)
            .map(|e| e.equipment)
            .next()
            .and_then(|e| weapons.get(e).ok());

        if let Some(weapon) = weapon {
            commands.entity(entity).insert(weapon.clone());
        } else if let Some(unarmed) = unarmed {
            commands.entity(entity).insert(unarmed.weapon.clone());
        } else {
            commands.entity(entity).remove::<MeleeWeapon>();
        }
    }
}

pub fn attack_current_target(
    mut damage_events: EventWriter<DamageDealt>,
    mut actors: Query<(Entity, &mut CurrentActivity, &mut AttackTarget, &MapPosition, &MeleeWeapon), With<Alive>>,
    mut targets: Query<&MapPosition, With<Alive>>,
) {
    for (entity, mut current_activity, current_target, map_position, weapon) in &mut actors {
        if !current_activity.is_idle() {
            continue;
        }

        let target_position = match targets.get_mut(current_target.target) {
            Ok(x) => x,
            _ => continue,
        };

        if !target_position.in_range(map_position, weapon.range) {
            continue;
        }

        damage_events.send(DamageDealt {
            target: current_target.target,
            source: entity,
            damage: weapon.damage,
        });
        *current_activity = CurrentActivity::Melee(Timer::new(weapon.delay, TimerMode::Once));
    }
}

pub fn apply_damage(
    mut commands: Commands,
    mut damage_events: EventReader<DamageDealt>,
    mut died_events: EventWriter<CharacterDied>,
    mut characters: Query<&mut Stats, With<Alive>>,
) {
    for event in &mut damage_events {
        let mut stats = match characters.get_mut(event.target) {
            Ok(x) => x,
            _ => continue,
        };

        stats.hp = stats.hp.saturating_sub(event.damage);
        if stats.hp > 0 {
            continue;
        }

        commands.entity(event.target).remove::<Alive>();
        died_events.send(CharacterDied {
            character: event.target,
        });
    }
}

pub fn remove_dead_characters(mut commands: Commands, mut events: EventReader<CharacterDied>) {
    for event in &mut events {
        commands.entity(event.character).despawn_recursive();
    }
}

pub fn spawn_corpses(
    mut commands: Commands,
    mut died_events: EventReader<CharacterDied>,
    mut corpse_events: EventWriter<CorpseSpawned>,
    entity_allocator: Res<NetEntityAllocator>,
    characters: Query<(&Character, &MapPosition)>,
) {
    for event in &mut died_events {
        let (character, map_position) = match characters.get(event.character) {
            Ok(x) => x,
            _ => continue,
        };

        let corpse = commands
            .spawn((
                NetEntity { id: entity_allocator.allocate_item() },
                *map_position,
                Flags::default(),
                Graphic {
                    id: CORPSE_GRAPHIC_ID,
                    hue: character.hue,
                },
                Quantity { quantity: character.body_type },
            ))
            .id();
        corpse_events.send(CorpseSpawned {
            character: event.character,
            corpse,
        });
    }
}

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_event::<CharacterDied>()
            .add_event::<CorpseSpawned>()
            .add_event::<DamageDealt>()
            .add_systems((
                handle_attack_requests,
                update_weapon_stats,
                attack_current_target
                    .after(progress_current_activity)
                    .after(update_weapon_stats),
                apply_damage,
                remove_dead_characters.after(apply_damage),
                spawn_corpses.after(apply_damage).before(remove_dead_characters),
            ));
    }
}
