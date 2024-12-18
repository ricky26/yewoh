use bevy::prelude::*;
use serde::Deserialize;
use std::time::Duration;
use yewoh_server::world::characters::{Animation, CharacterBodyType, Health, OnCharacterAnimationStart};
use yewoh_server::world::combat::{AttackTarget, OnCharacterDamage, OnCharacterSwing, OnClientAttackRequest};
use yewoh_server::world::connection::Possessing;
use yewoh_server::world::entity::{EquipmentSlot, EquippedPosition, MapPosition};
use yewoh_server::world::net_id::NetId;
use yewoh_server::world::ServerSet;

use crate::activities::{progress_current_activity, CurrentActivity};
use crate::characters::corpses::{spawn_corpses, OnCharacterDeath};

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Component)]
pub struct Invulnerable;

#[derive(Debug, Clone, Event)]
pub struct OnDealMeleeDamage {
    pub target: Entity,
    pub source: Entity,
    pub damage: u16,
    pub location: MapPosition,
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


pub fn on_client_attack_request(
    mut commands: Commands,
    clients: Query<&Possessing>,
    mut events: EventReader<OnClientAttackRequest>,
) {
    for request in events.read() {
        let Ok(possessing) = clients.get(request.client_entity) else {
            continue;
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
    mut commands: Commands,
    characters: Query<(), With<CharacterBodyType>>,
    weapons: Query<
        (&Parent, &EquippedPosition, &MeleeWeapon),
        (Without<CharacterBodyType>, Or<(Changed<NetId>, Changed<EquippedPosition>, Changed<MeleeWeapon>)>,
    )>,
) {
    for (parent, equipped, weapon) in &weapons {
        if equipped.slot != EquipmentSlot::MainHand {
            continue;
        };

        if !characters.contains(parent.get()) {
            continue;
        };

        commands.entity(parent.get()).insert(weapon.clone());
    }
}

pub fn attack_current_target(
    mut damage_events: EventWriter<OnDealMeleeDamage>,
    mut animation_events: EventWriter<OnCharacterAnimationStart>,
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

        animation_events.send(OnCharacterAnimationStart {
            animation: weapon.swing_animation.clone(),
            entity,
            location: *location,
        });

        if let Some(animation) = hit_animation.cloned() {
            animation_events.send(OnCharacterAnimationStart {
                animation: animation.hit_animation,
                entity: current_target.target,
                location: *target_location,
            });
        }

        damage_events.send(OnDealMeleeDamage {
            target: current_target.target,
            source: entity,
            damage: weapon.min_damage,
            location: *target_location,
        });

        *current_activity = CurrentActivity::Melee(Timer::new(weapon.delay, TimerMode::Once));
    }
}

pub fn apply_damage(
    mut damage_events: EventReader<OnDealMeleeDamage>,
    mut died_events: EventWriter<OnCharacterDeath>,
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

        died_events.send(OnCharacterDeath {
            character: event.target,
        });
    }
}

pub fn send_damage_notices(
    mut in_damage_events: EventReader<OnDealMeleeDamage>,
    mut out_damage_events: EventWriter<OnCharacterDamage>,
    mut out_swing_events: EventWriter<OnCharacterSwing>,
) {
    for event in in_damage_events.read() {
        out_damage_events.send(OnCharacterDamage {
            target: event.target,
            damage: event.damage,
        });

        out_swing_events.send(OnCharacterSwing {
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
            .add_event::<OnDealMeleeDamage>()
            .add_systems(First, (
                (
                    on_client_attack_request,
                ).in_set(ServerSet::HandlePackets),
            ))
            .add_systems(Update, (
                update_weapon_stats,
                update_weapon_stats_on_equip,
                attack_current_target
                    .after(progress_current_activity)
                    .after(update_weapon_stats),
                (
                    apply_damage.before(spawn_corpses),
                    send_damage_notices,
                ).after(attack_current_target),
            ));
    }
}
