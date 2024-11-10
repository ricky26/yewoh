use bevy::prelude::*;
use bevy::reflect::reflect_remote;

#[reflect_remote(yewoh::protocol::EquipmentSlot)]
#[derive(Default)]
#[reflect(Default)]
pub enum EquipmentSlot {
    Invalid,
    MainHand,
    OffHand,
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

#[reflect_remote(yewoh::Notoriety)]
#[derive(Default)]
#[reflect(Default)]
pub enum Notoriety {
    Innocent,
    Friend,
    Neutral,
    Criminal,
    Enemy,
    Murderer,
    Invulnerable,
}

#[reflect_remote(yewoh::Direction)]
#[derive(Default)]
#[reflect(Default)]
pub enum Direction {
    North,
    Right,
    East,
    Down,
    South,
    Left,
    West,
    Up,
}

#[reflect_remote(yewoh::protocol::Race)]
#[derive(Default)]
#[reflect(Default)]
pub enum Race {
    Human,
    Elf,
    Gargoyle,
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<EquipmentSlot>()
        .register_type::<Notoriety>()
        .register_type::<Direction>()
        .register_type::<Race>();
}
