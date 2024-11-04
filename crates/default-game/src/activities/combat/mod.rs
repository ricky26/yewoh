use std::time::Duration;
use bevy::prelude::*;
use serde::Deserialize;
use yewoh::protocol::EquipmentSlot;
use yewoh_server::world::characters::{Animation, AnimationStartedEvent, CharacterBodyType, Health};
use yewoh_server::world::combat::{AttackRequestedEvent, AttackTarget, DamagedEvent, SwingEvent};
use yewoh_server::world::connection::Possessing;
use yewoh_server::world::entity::{EquippedPosition, Hue, MapPosition};
use yewoh_server::world::items::{Container, ItemGraphic, ItemQuantity};

use crate::activities::{progress_current_activity, CurrentActivity};

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Component)]
pub struct Invulnerable;

#[derive(Debug, Clone, Event)]
pub struct MeleeDamageDealtEvent {
    pub target: Entity,
    pub source: Entity,
    pub damage: u16,
    pub location: MapPosition,
}

#[derive(Debug, Clone, Event)]
pub struct CharacterDied {
    pub character: Entity,
}

#[derive(Debug, Default, Clone, Component)]
pub struct Corpse;

#[derive(Debug, Clone, Event)]
pub struct CorpseSpawned {
    pub character: Entity,
    pub corpse: Entity,
}

#[derive(Debug, Clone, Default, Reflect, Component)]
#[reflect(Component)]
pub struct HitAnimation {
    pub hit_animation: Animation,
}

#[derive(Debug, Clone, Default, Reflect, Component, Deserialize)]
#[reflect(Component, Deserialize)]
pub struct MeleeWeapon {
    pub min_damage: u16,
    pub max_damage: u16,
    #[serde(with = "humantime_serde")]
    pub delay: Duration,
    pub range: i32,
    pub swing_animation: Animation,
}

#[derive(Debug, Clone, Reflect, Component)]
#[reflect(Component)]
pub struct Unarmed {
    pub weapon: MeleeWeapon,
}

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
        (With<CharacterBodyType>, Or<(Changed<CharacterBodyType>, Changed<Children>, Changed<Unarmed>)>),
    >,
    weapons: Query<&EquippedPosition>,
) {
    for (entity, children, unarmed) in &mut characters {
        let Some(children) = children else {
            commands.entity(entity).remove::<MeleeWeapon>();
            continue;
        };

        let has_weapon = children.iter()
            .filter_map(|e| weapons.get(*e).ok())
            .any(|w| w.slot == EquipmentSlot::MainHand);
        if has_weapon {
            // MeleeWeapon will have already been populated by the weapon.
            continue;
        }

        if let Some(unarmed) = unarmed {
            commands.entity(entity).insert(unarmed.weapon.clone());
        } else {
            commands.entity(entity).remove::<MeleeWeapon>();
        }
    }
}

pub fn update_weapon_stats_on_equip(
    mut characters: Query<&mut MeleeWeapon, With<CharacterBodyType>>,
    weapons: Query<
        (&Parent, &EquippedPosition, &MeleeWeapon),
        (Without<CharacterBodyType>, Or<(Changed<EquippedPosition>, Changed<MeleeWeapon>)>,
    )>,
) {
    for (parent, equipped, weapon) in &weapons {
        if equipped.slot != EquipmentSlot::MainHand {
            continue;
        };

        let Ok(mut out_weapon) = characters.get_mut(parent.get()) else {
            continue;
        };

        *out_weapon = weapon.clone();
    }
}

pub fn attack_current_target(
    mut damage_events: EventWriter<MeleeDamageDealtEvent>,
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

        damage_events.send(MeleeDamageDealtEvent {
            target: current_target.target,
            source: entity,
            damage: weapon.min_damage,
            location: *target_location,
        });

        *current_activity = CurrentActivity::Melee(Timer::new(weapon.delay, TimerMode::Once));
    }
}

pub fn apply_damage(
    mut damage_events: EventReader<MeleeDamageDealtEvent>,
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
    mut in_damage_events: EventReader<MeleeDamageDealtEvent>,
    mut out_damage_events: EventWriter<DamagedEvent>,
    mut out_swing_events: EventWriter<SwingEvent>,
) {
    for event in in_damage_events.read() {
        out_damage_events.send(DamagedEvent {
            target: event.target,
            damage: event.damage,
        });

        out_swing_events.send(SwingEvent {
            target: event.target,
            attacker: event.source,
        });
    }
}

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app
            .register_type::<Invulnerable>()
            .register_type::<HitAnimation>()
            .register_type::<MeleeWeapon>()
            .register_type::<Unarmed>()
            .add_event::<CharacterDied>()
            .add_event::<CorpseSpawned>()
            .add_event::<MeleeDamageDealtEvent>()
            .add_systems(Update, (
                handle_attack_requests,
                update_weapon_stats,
                update_weapon_stats_on_equip,
                attack_current_target
                    .after(progress_current_activity)
                    .after(update_weapon_stats),
                (
                    apply_damage,
                    send_damage_notices,
                ).after(attack_current_target),
                remove_dead_characters.after(apply_damage),
                spawn_corpses.after(apply_damage).before(remove_dead_characters),
            ));
    }
}
