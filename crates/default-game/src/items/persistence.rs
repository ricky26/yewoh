use std::fmt::Formatter;

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{FromWorld, World};
use bevy_ecs::query::{With, WorldQuery};
use glam::IVec2;
use serde::{Deserializer, Serialize, Serializer};
use serde::de::{DeserializeSeed, Error, MapAccess, Visitor};
use serde::ser::SerializeMap;

use yewoh::protocol::EquipmentSlot;
use yewoh_server::world::entity::{Container, EquippedBy, Flags, Graphic, Location, ParentContainer};

use crate::entities::Persistent;
use crate::persistence::{BundleSerializer, DeserializeContext, SerializeContext};
use crate::persistence::entity::{EntityListSerializer, EntityListVisitor};

pub struct ItemSerializer;

impl FromWorld for ItemSerializer {
    fn from_world(_world: &mut World) -> Self {
        Self
    }
}

impl BundleSerializer for ItemSerializer {
    type Query = (
        &'static Graphic,
        &'static Flags,
        Option<&'static Location>,
        Option<&'static Container>,
        Option<&'static ParentContainer>,
        Option<&'static EquippedBy>,
    );
    type Filter = With<Persistent>;
    type Bundle = (
        Graphic,
        Flags,
        Option<Location>,
        Option<Container>,
        Option<ParentContainer>,
        Option<EquippedBy>,
    );

    fn id() -> &'static str {
        "Item"
    }

    fn extract(item: <Self::Query as WorldQuery>::Item<'_>) -> Self::Bundle {
        let (
            graphic,
            flags,
            location,
            container,
            parent_container,
            equipped_by,
        ) = item.clone();
        (
            graphic.clone(),
            flags.clone(),
            location.cloned(),
            container.cloned(),
            parent_container.cloned(),
            equipped_by.cloned(),
        )
    }

    fn serialize<S: Serializer>(ctx: &SerializeContext, s: S, bundle: &Self::Bundle) -> Result<S::Ok, S::Error> {
        let (
            graphic,
            flags,
            location,
            container,
            parent_container,
            equipped_by,
        ) = bundle;
        let mut map = s.serialize_map(None)?;
        map.serialize_entry("graphic", graphic)?;
        map.serialize_entry("flags", flags)?;

        if let Some(location) = location {
            map.serialize_entry("location", location)?;
        }

        if let Some(container) = container {
            map.serialize_entry("container", &ContainerSerializer { ctx, container })?;
        }

        if let Some(parent_container) = parent_container {
            map.serialize_entry("parent_container", &ParentContainerSerializer { ctx, parent_container })?;
        }

        if let Some(equipped_by) = equipped_by {
            map.serialize_entry("equipped_by", &EquippedBySerializer { ctx, equipped_by })?;
        }

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
                write!(formatter, "item bundle")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error> where A: MapAccess<'de> {
                let ctx = self.ctx;
                let entity = self.entity;
                let mut graphic = None;
                let mut flags = Flags::default();
                let mut location = None;
                let mut container = None;
                let mut parent_container = None;
                let mut equipped_by = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "graphic" => graphic = Some(map.next_value::<Graphic>()?),
                        "flags" => flags = map.next_value()?,
                        "location" => location = Some(map.next_value::<Location>()?),
                        "container" => container = Some(map.next_value_seed(ContainerVisitor { ctx })?),
                        "parent_container" => parent_container = Some(map.next_value_seed(ParentContainerVisitor { ctx })?),
                        "equipped_by" => equipped_by = Some(map.next_value_seed(EquippedByVisitor { ctx })?),
                        name => return Err(A::Error::unknown_field(name, &["graphic", "flags", "container", "parent_container", "equipped_by"])),
                    }
                }

                let graphic = match graphic {
                    Some(x) => x,
                    None => return Err(A::Error::missing_field("graphic")),
                };

                let mut entity_ref = ctx.world_mut().entity_mut(entity);
                entity_ref.insert((graphic, flags, Persistent));

                if let Some(location) = location {
                    entity_ref.insert(location);
                }

                if let Some(container) = container {
                    entity_ref.insert(container);
                }

                if let Some(parent_container) = parent_container {
                    entity_ref.insert(parent_container);
                }

                if let Some(equipped_by) = equipped_by {
                    entity_ref.insert(equipped_by);
                }

                Ok(())
            }
        }

        d.deserialize_map(BundleVisitor { ctx, entity })
    }
}

struct ContainerVisitor<'a, 'w> {
    ctx: &'a mut DeserializeContext<'w>,
}

impl<'a, 'w, 'de> Visitor<'de> for ContainerVisitor<'a, 'w> {
    type Value = Container;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "container")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error> where A: MapAccess<'de> {
        let mut gump_id = None;
        let mut items = Vec::new();

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "gump_id" => gump_id = Some(map.next_value()?),
                "items" => items = map.next_value_seed(EntityListVisitor::new(self.ctx))?,
                name => return Err(A::Error::unknown_field(name, &["gump_id", "items"])),
            }
        }

        let gump_id = match gump_id {
            Some(x) => x,
            None => return Err(A::Error::missing_field("gump_id")),
        };

        Ok(Container { gump_id, items })
    }
}

impl<'a, 'w, 'de> DeserializeSeed<'de> for ContainerVisitor<'a, 'w> {
    type Value = Container;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_map(self)
    }
}

struct ContainerSerializer<'a> {
    ctx: &'a SerializeContext,
    container: &'a Container,
}

impl<'a> Serialize for ContainerSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("gump_id", &self.container.gump_id)?;
        map.serialize_entry("items", &EntityListSerializer::new(self.ctx, &self.container.items))?;
        map.end()
    }
}

struct ParentContainerVisitor<'a, 'w> {
    ctx: &'a mut DeserializeContext<'w>,
}

impl<'a, 'w, 'de> Visitor<'de> for ParentContainerVisitor<'a, 'w> {
    type Value = ParentContainer;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "container")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error> where A: MapAccess<'de> {
        let mut parent = None;
        let mut position = IVec2::ZERO;
        let mut grid_index = 0;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "parent" => parent = Some(self.ctx.map_entity(map.next_value()?)),
                "position" => position = map.next_value()?,
                "grid_index" => grid_index = map.next_value()?,
                name => return Err(A::Error::unknown_field(name, &["parent", "position", "grid_index"])),
            }
        }

        let parent = match parent {
            Some(x) => x,
            None => return Err(A::Error::missing_field("parent")),
        };

        Ok(ParentContainer { parent, position, grid_index })
    }
}

impl<'a, 'w, 'de> DeserializeSeed<'de> for ParentContainerVisitor<'a, 'w> {
    type Value = ParentContainer;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_map(self)
    }
}

struct ParentContainerSerializer<'a> {
    ctx: &'a SerializeContext,
    parent_container: &'a ParentContainer,
}

impl<'a> Serialize for ParentContainerSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("parent", &self.ctx.map_entity(self.parent_container.parent))?;
        map.serialize_entry("position", &self.parent_container.position)?;
        map.serialize_entry("grid_index", &self.parent_container.grid_index)?;
        map.end()
    }
}

struct EquippedByVisitor<'a, 'w> {
    ctx: &'a mut DeserializeContext<'w>,
}

impl<'a, 'w, 'de> Visitor<'de> for EquippedByVisitor<'a, 'w> {
    type Value = EquippedBy;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "container")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error> where A: MapAccess<'de> {
        let mut parent = None;
        let mut slot = EquipmentSlot::MainHand;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "parent" => parent = Some(self.ctx.map_entity(map.next_value()?)),
                "slot" => slot = map.next_value()?,
                name => return Err(A::Error::unknown_field(name, &["parent", "slot"])),
            }
        }

        let parent = match parent {
            Some(x) => x,
            None => return Err(A::Error::missing_field("parent")),
        };

        Ok(EquippedBy { parent, slot })
    }
}

impl<'a, 'w, 'de> DeserializeSeed<'de> for EquippedByVisitor<'a, 'w> {
    type Value = EquippedBy;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_map(self)
    }
}

struct EquippedBySerializer<'a> {
    ctx: &'a SerializeContext,
    equipped_by: &'a EquippedBy,
}

impl<'a> Serialize for EquippedBySerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("parent", &self.ctx.map_entity(self.equipped_by.parent))?;
        map.serialize_entry("slot", &self.equipped_by.slot)?;
        map.end()
    }
}
