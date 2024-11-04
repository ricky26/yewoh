use bevy::prelude::*;

use yewoh::protocol;
use yewoh::protocol::EquipmentSlot;
use yewoh_server::world::characters::{AnimationStartedEvent, CharacterBodyType, Health};
use yewoh_server::world::combat::{AttackRequestedEvent, AttackTarget};
use yewoh_server::world::connection::{NetClient, Possessing};
use yewoh_server::world::entity::{EquippedPosition, Hue, MapPosition};
use yewoh_server::world::items::{Container, ItemGraphic, ItemQuantity};
use yewoh_server::world::net_id::NetId;
use yewoh_server::world::ServerSet;

use crate::activities::{progress_current_activity, CurrentActivity};
use crate::characters::{CharacterDied, Corpse, CorpseSpawned, DamageDealt, HitAnimation, Invulnerable, MeleeWeapon, Unarmed};

mod prefabs;

pub const CORPSE_GRAPHIC_ID: u16 = 0x2006;
pub const CORPSE_BOX_GUMP_ID: u16 = 9;

pub fn handle_attack_requests(
    mut commands: Commands,
    mut requests: EventReader<AttackRequestedEvent>,
    clients: Query<&Possessing>,
) {
    for request in requests.read() {
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
    mut characters: Query<
        (Entity, Option<&Children>, Option<&Unarmed>),
        (With<CharacterBodyType>, Or<(Changed<CharacterBodyType>, Changed<Unarmed>)>),
    >,
    weapons: Query<(&EquippedPosition, &MeleeWeapon)>,
) {
    for (entity, children, unarmed) in &mut characters {
        let Some(children) = children else {
            commands.entity(entity).remove::<MeleeWeapon>();
            continue;
        };

        let weapon = children.iter()
            .filter_map(|e| match weapons.get(*e)
            {
                Ok((equipped, weapon)) => Some((*e, equipped, weapon)),
                _ => None,
            })
            .filter(|(_, pos, _)| pos.slot == EquipmentSlot::MainHand)
            .map(|e| e.2)
            .next();

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
    mut actors: Query<
        (Entity, &mut CurrentActivity, &mut AttackTarget, &MapPosition, &MeleeWeapon),
        Without<Invulnerable>,
    >,
    mut targets: Query<(&MapPosition, Option<&HitAnimation>), Without<Invulnerable>>,
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
    mut damage_events: EventReader<DamageDealt>,
    mut died_events: EventWriter<CharacterDied>,
    mut characters: Query<&mut Health, Without<Invulnerable>>,
) {
    for event in damage_events.read() {
        let mut health = match characters.get_mut(event.target) {
            Ok(x) => x,
            _ => continue,
        };

        health.hp = health.hp.saturating_sub(event.damage);
        if health.hp > 0 {
            continue;
        }

        died_events.send(CharacterDied {
            character: event.target,
        });
    }
}

pub fn remove_dead_characters(mut commands: Commands, mut events: EventReader<CharacterDied>) {
    for event in events.read() {
        commands.entity(event.character).despawn_recursive();
    }
}

pub fn spawn_corpses(
    mut commands: Commands,
    mut died_events: EventReader<CharacterDied>,
    mut corpse_events: EventWriter<CorpseSpawned>,
    characters: Query<(&CharacterBodyType, &Hue, &MapPosition)>,
) {
    for event in died_events.read() {
        let (body_type, hue, map_position) = match characters.get(event.character) {
            Ok(x) => x,
            _ => continue,
        };

        let corpse = commands
            .spawn((
                *map_position,
                ItemGraphic(CORPSE_GRAPHIC_ID),
                ItemQuantity(**body_type),
                Hue(**hue),
                Container {
                    gump_id: CORPSE_BOX_GUMP_ID,
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
    net_ids: Query<&NetId>,
    clients: Query<&NetClient>,
    mut damage_events: EventReader<DamageDealt>,
) {
    for event in damage_events.read() {
        let target_id = match net_ids.get(event.target) {
            Ok(x) => x.id,
            _ => continue,
        };

        let attacker_id = net_ids.get(event.source)
            .ok()
            .map(|id| id.id);

        // TODO: filter clients
        for client in &clients {
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
            .register_type::<prefabs::MeleeWeaponPrefab>()
            .add_event::<CharacterDied>()
            .add_event::<CorpseSpawned>()
            .add_event::<DamageDealt>()
            .add_systems(Update, (
                handle_attack_requests,
                update_weapon_stats,
                attack_current_target
                    .after(progress_current_activity)
                    .after(update_weapon_stats),
                apply_damage,
                remove_dead_characters.after(apply_damage),
                spawn_corpses.after(apply_damage).before(remove_dead_characters),
            ))
            .add_systems(Last, (
                send_damage_notices,
            ).in_set(ServerSet::Send));
    }
}
