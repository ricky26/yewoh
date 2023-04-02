use bevy_app::{App, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::event::{EventReader, EventWriter};
use bevy_ecs::query::{Changed, With};
use bevy_ecs::schedule::{IntoSystemConfig, IntoSystemConfigs};
use bevy_ecs::system::{Commands, Query, Res};
use bevy_time::{Timer, TimerMode};

use yewoh::protocol;
use yewoh::protocol::EquipmentSlot;
use yewoh_server::world::entity::{AttackTarget, Character, Container, Flags, Graphic, Location, Quantity, Stats};
use yewoh_server::world::events::AttackRequestedEvent;
use yewoh_server::world::hierarchy::DespawnRecursiveExt;
use yewoh_server::world::net::{NetClient, NetEntity, NetEntityAllocator, NetEntityLookup, Possessing};
use yewoh_server::world::ServerSet;
use yewoh_server::world::spatial::NetClientPositions;

use crate::activities::{CurrentActivity, progress_current_activity};
use crate::characters::{Alive, CharacterDied, Corpse, CorpseSpawned, DamageDealt, HitAnimation, MeleeWeapon, Unarmed};
use crate::characters::animation::AnimationStartedEvent;

pub const CORPSE_GRAPHIC_ID: u16 = 0x2006;
pub const CORPSE_BOX_GUMP_ID: u16 = 9;

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
    mut animation_events: EventWriter<AnimationStartedEvent>,
    mut actors: Query<(Entity, &mut CurrentActivity, &mut AttackTarget, &Location, &MeleeWeapon), With<Alive>>,
    mut targets: Query<(&Location, Option<&HitAnimation>), With<Alive>>,
) {
    for (entity, mut current_activity, current_target, location, weapon) in &mut actors {
        if !current_activity.is_idle() {
            continue;
        }

        let (target_location, hit_animation) = match targets.get_mut(current_target.target) {
            Ok(x) => x,
            _ => continue,
        };

        if !target_location.in_range(location, weapon.range) {
            continue;
        }

        animation_events.send(AnimationStartedEvent {
            animation: weapon.swing_animation.clone(),
            entity,
            location: *location,
        });

        if let Some(animation) = hit_animation.cloned() {
            animation_events.send(AnimationStartedEvent {
                animation: animation.hit_animation,
                entity: current_target.target,
                location: *target_location,
            });
        }

        damage_events.send(DamageDealt {
            target: current_target.target,
            source: entity,
            damage: weapon.damage,
            location: *target_location,
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
    characters: Query<(&Character, &Location)>,
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
                Container {
                    gump_id: CORPSE_BOX_GUMP_ID,
                    items: vec![],
                },
                Corpse,
            ))
            .id();
        corpse_events.send(CorpseSpawned {
            character: event.character,
            corpse,
        });
    }
}

pub fn send_damage_notices(
    entity_lookup: Res<NetEntityLookup>,
    client_positions: Res<NetClientPositions>,
    mut damage_events: EventReader<DamageDealt>,
    clients: Query<&NetClient>,
) {
    for event in &mut damage_events {
        let target_id = match entity_lookup.ecs_to_net(event.target) {
            Some(x) => x,
            None => continue,
        };

        let attacker_id = entity_lookup.ecs_to_net(event.source);

        for (entity, ..) in client_positions.tree.iter_at_point(event.location.map_id, event.location.position.truncate()) {
            let client = match clients.get(entity) {
                Ok(x) => x,
                _ => continue,
            };


            client.send_packet(protocol::DamageDealt {
                target_id,
                damage: event.damage,
            }.into());

            if let Some(attacker_id) = attacker_id {
                client.send_packet(protocol::Swing {
                    attacker_id,
                    target_id,
                }.into());
            }
        }
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
            ))
            .add_systems((
                send_damage_notices,
            ).in_set(ServerSet::Send));
    }
}
