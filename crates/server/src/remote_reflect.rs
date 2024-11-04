use bevy::prelude::*;
use bevy::reflect::reflect_remote;
use yewoh::protocol::{EntityFlags, EquipmentSlot, Race};
use yewoh::{Direction, Notoriety};

#[reflect_remote(EquipmentSlot)]
#[derive(Default)]
#[reflect(Default)]
pub enum EquipmentSlotRemote {
    Invalid,
    MainHand,
    BothHands,
    Shoes,
    Bottom,
    Top,
    Head,
    Hands,
    Ring,
    Talisman,
    Neck,
    Hair,
    Waist,
    InnerTorso,
    Bracelet,
    FacialHair,
    MiddleTorso,
    Earrings,
    Arms,
    Cloak,
    Backpack,
    OuterTorso,
    OuterLegs,
    InnerLegs,
    Mount,
    ShopBuy,
    ShopBuyback,
    ShopSell,
    Bank,
}

#[reflect_remote(Notoriety)]
#[derive(Default)]
#[reflect(Default)]
pub enum NotorietyRemote {
    Innocent,
    Friend,
    Neutral,
    Criminal,
    Enemy,
    Murderer,
    Invulnerable,
}

#[reflect_remote(Direction)]
#[derive(Default)]
#[reflect(Default)]
pub enum DirectionRemote {
    North,
    Right,
    East,
    Down,
    South,
    Left,
    West,
    Up,
}

#[reflect_remote(EntityFlags)]
#[derive(Default)]
#[reflect(Default)]
pub struct EntityFlagsRemote;

#[reflect_remote(Race)]
#[derive(Default)]
#[reflect(Default)]
pub enum RaceRemote {
    Human,
    Elf,
    Gargoyle,
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<EquipmentSlotRemote>()
        .register_type::<NotorietyRemote>()
        .register_type::<DirectionRemote>()
        .register_type::<EntityFlagsRemote>()
        .register_type::<RaceRemote>();
}