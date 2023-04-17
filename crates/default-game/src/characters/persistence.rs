use std::fmt::Formatter;

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{FromWorld, World};
use bevy_ecs::query::{With, WorldQuery};
use serde::{Deserializer, Serialize, Serializer};
use serde::de::{DeserializeSeed, Error, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};

use yewoh::protocol::EquipmentSlot;
use yewoh_server::world::entity::{Character, CharacterEquipped, Flags, Location, Stats};
use yewoh_server::world::net::NetCommandsExt;

use crate::characters::Alive;
use crate::entities::Persistent;
use crate::persistence::{BundleSerializer, DeserializeContext, EntityReference, SerializeContext};

pub struct CharacterSerializer;

impl FromWorld for CharacterSerializer {
    fn from_world(_world: &mut World) -> Self {
        Self
    }
}

impl BundleSerializer for CharacterSerializer {
    type Query = (
        &'static Character,
        &'static Flags,
        &'static Stats,
        &'static Location,
    );
    type Filter = With<Persistent>;
    type Bundle = (
        Character,
        Flags,
        Stats,
        Location,
    );

    fn id() -> &'static str {
        "Character"
    }

    fn extract(item: <Self::Query as WorldQuery>::Item<'_>) -> Self::Bundle {
        let (
            character,
            flags,
            stats,
            location,
        ) = item.clone();
        (
            character.clone(),
            flags.clone(),
            stats.clone(),
            location.clone(),
        )
    }

    fn serialize<S: Serializer>(ctx: &SerializeContext, s: S, bundle: &Self::Bundle) -> Result<S::Ok, S::Error> {
        let (
            character,
            flags,
            stats,
            location,
        ) = bundle;
        let mut map = s.serialize_map(None)?;
        map.serialize_entry("stats", stats)?;
        map.serialize_entry("flags", flags)?;
        map.serialize_entry("location", location)?;
        map.serialize_entry("character", &CharacterComponentSerializer { ctx, character })?;
        map.end()
    }

    fn deserialize<'de, D: Deserializer<'de>>(ctx: &mut DeserializeContext, d: D, entity: Entity) -> Result<(), D::Error> {
        struct BundleVisitor<'a, 'w> {
            ctx: &'a mut DeserializeContext<'w>,
            entity: Entity,
        }

        impl<'a, 'w, 'de> Visitor<'de> for BundleVisitor<'a, 'w> {
            type Value = ();

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                write!(formatter, "character bundle")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error> where A: MapAccess<'de> {
                let ctx = self.ctx;
                let entity = self.entity;
                ctx.world_mut().entity_mut(entity)
                    .insert(Persistent);

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "stats" => {
                            let stats = map.next_value::<Stats>()?;

                            let mut entity_ref = ctx.world_mut()
                                .entity_mut(entity);

                            if stats.hp > 0 {
                                entity_ref.insert(Alive);
                            }

                            entity_ref.insert(stats);
                        }
                        "flags" => {
                            ctx.world_mut()
                                .entity_mut(entity)
                                .insert(map.next_value::<Flags>()?);
                        }
                        "location" => {
                            ctx.world_mut()
                                .entity_mut(entity)
                                .insert(map.next_value::<Location>()?);
                        }
                        "character" => {
                            let bundle = map.next_value_seed(CharacterVisitor { ctx })?;
                            ctx.world_mut()
                                .entity_mut(entity)
                                .insert((bundle, Persistent));
                        }
                        name => return Err(A::Error::unknown_field(name, &["character", "stats", "flags", "location"])),
                    };
                }

                ctx.world_mut().entity_mut(entity).assign_network_id();
                Ok(())
            }
        }

        d.deserialize_map(BundleVisitor { ctx, entity })
    }
}

struct CharacterVisitor<'a, 'w> {
    ctx: &'a mut DeserializeContext<'w>,
}

impl<'a, 'w, 'de> Visitor<'de> for CharacterVisitor<'a, 'w> {
    type Value = Character;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "character")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error> where A: MapAccess<'de> {
        let mut body_type = 0u16;
        let mut hue = 0u16;
        let mut equipment = Vec::new();

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "body_type" => body_type = map.next_value()?,
                "hue" => hue = map.next_value()?,
                "equipment" => equipment = map.next_value_seed(EquipmentListVisitor { ctx: self.ctx })?,
                name => return Err(A::Error::unknown_field(name, &["body_type", "hue", "equipment"])),
            }
        }

        Ok(Character { body_type, hue, equipment })
    }
}

impl<'a, 'w, 'de> DeserializeSeed<'de> for CharacterVisitor<'a, 'w> {
    type Value = Character;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_map(self)
    }
}

struct EquipmentListVisitor<'a, 'w> {
    ctx: &'a mut DeserializeContext<'w>,
}

impl<'a, 'w, 'de> Visitor<'de> for EquipmentListVisitor<'a, 'w> {
    type Value = Vec<CharacterEquipped>;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "equipment list")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
        let mut result = Vec::new();
        while let Some(item) = seq.next_element_seed(EquipmentVisitor { ctx: self.ctx })? {
            result.push(item);
        }
        Ok(result)
    }
}

impl<'a, 'w, 'de> DeserializeSeed<'de> for EquipmentListVisitor<'a, 'w> {
    type Value = Vec<CharacterEquipped>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_seq(self)
    }
}

struct EquipmentVisitor<'a, 'w> {
    ctx: &'a mut DeserializeContext<'w>,
}

impl<'a, 'w, 'de> Visitor<'de> for EquipmentVisitor<'a, 'w> {
    type Value = CharacterEquipped;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "equipped item")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error> where A: MapAccess<'de> {
        let mut equipment = None;
        let mut slot = EquipmentSlot::MainHand;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "equipment" => equipment = Some(self.ctx.map_entity(map.next_value::<EntityReference>()?)),
                "slot" => slot = map.next_value()?,
                name => return Err(A::Error::unknown_field(name, &["equipment", "slot"])),
            }
        }

        if let Some(equipment) = equipment {
            Ok(CharacterEquipped { entity: equipment, slot })
        } else {
            Err(A::Error::custom("missing equipment"))
        }
    }
}

impl<'a, 'w, 'de> DeserializeSeed<'de> for EquipmentVisitor<'a, 'w> {
    type Value = CharacterEquipped;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_map(self)
    }
}

struct CharacterComponentSerializer<'a> {
    ctx: &'a SerializeContext,
    character: &'a Character,
}

impl<'a> Serialize for CharacterComponentSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("body_type", &self.character.body_type)?;
        map.serialize_entry("hue", &self.character.hue)?;
        map.serialize_entry("equipment", &EquipmentListSerializer { ctx: self.ctx, equipped: &self.character.equipment })?;
        map.end()
    }
}

struct EquipmentListSerializer<'a> {
    ctx: &'a SerializeContext,
    equipped: &'a [CharacterEquipped],
}

impl<'a> Serialize for EquipmentListSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut seq = serializer.serialize_seq(Some(self.equipped.len()))?;
        for item in self.equipped {
            seq.serialize_element(&EquipmentSerializer { ctx: self.ctx, equipment: item })?;
        }
        seq.end()
    }
}

struct EquipmentSerializer<'a> {
    ctx: &'a SerializeContext,
    equipment: &'a CharacterEquipped,
}

impl<'a> Serialize for EquipmentSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("equipment", &self.ctx.map_entity(self.equipment.entity))?;
        map.serialize_entry("slot", &self.equipment.slot)?;
        map.end()
    }
}
