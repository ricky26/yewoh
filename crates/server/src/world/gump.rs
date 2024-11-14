use bevy::ecs::entity::{EntityHashMap, VisitEntities, VisitEntitiesMut};
use bevy::ecs::reflect::ReflectMapEntities;
use bevy::prelude::*;
use bevy::utils::HashMap;
use smallvec::SmallVec;
use yewoh::protocol::{AnyPacket, GumpLayout, OpenGump};

use crate::world::connection::NetClient;
use crate::world::ServerSet;

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Default, Component)]
pub struct Gump {
    pub type_id: u32,
    pub position: IVec2,
    pub layout: String,
    pub text: Vec<String>,
}

impl Gump {
    pub fn empty(type_id: u32) -> Gump {
        Gump {
            type_id,
            position: IVec2::ZERO,
            layout: String::new(),
            text: Vec::new(),
        }
    }

    pub fn to_packet(&self, gump_id: u32) -> AnyPacket {
        OpenGump {
            gump_id,
            type_id: self.type_id,
            position: self.position,
            layout: GumpLayout {
                layout: self.layout.clone(),
                text: self.text.clone(),
            },
        }.into()
    }

    pub fn set_layout(&mut self, layout: GumpLayout) {
        self.layout = layout.layout;
        self.text = layout.text;
    }
}

#[derive(Clone, Copy, Debug, Deref, DerefMut, Reflect, Component, VisitEntities, VisitEntitiesMut)]
#[reflect(Component, MapEntities)]
pub struct GumpClient(pub Entity);

#[derive(Clone, Copy, Debug, Deref, DerefMut, Reflect, Component)]
#[reflect(Component)]
pub struct GumpId(pub u32);

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Default, Component)]
pub struct GumpIdAllocator {
    next_id: u32,
    free: Vec<u32>,
}

#[derive(Clone, Debug, Default, Reflect, Component)]
#[reflect(Default, Component)]
pub struct GumpSent;

impl GumpIdAllocator {
    pub fn allocate(&mut self) -> GumpId {
        let id = if let Some(id) = self.free.pop() {
            id
        } else {
            self.next_id += 1;
            self.next_id
        };
        GumpId(id)
    }

    pub fn free(&mut self, id: u32) {
        self.free.push(id);
    }
}

#[derive(Clone, Debug, Default, Reflect, Resource)]
#[reflect(Default, Resource)]
pub struct GumpLookup {
    id_from_gump: EntityHashMap<(Entity, u32, u32)>,
    gump_from_id: HashMap<(Entity, u32, u32), Entity>,
}

impl GumpLookup {
    pub fn get_gump(&self, client_entity: Entity, gump_id: u32, type_id: u32) -> Option<Entity> {
        self.gump_from_id.get(&(client_entity, gump_id, type_id)).copied()
    }

    pub fn get_gump_id(&self, gump: Entity) -> Option<(Entity, u32, u32)> {
        self.id_from_gump.get(&gump).copied()
    }

    pub fn insert(
        &mut self,
        commands: &mut Commands,
        gump_entity: Entity,
        client_entity: Entity,
        gump_id: u32,
        type_id: u32,
    ) {
        if let Some((old_client, old_gump_id, old_type_id)) = self.id_from_gump.insert(gump_entity, (client_entity, gump_id, type_id)) {
            if old_client == client_entity && old_gump_id == gump_id && old_type_id == type_id {
                // Already up to date.
                return;
            }

            // We replaced an existing gump!
            warn!("changed gump ID {gump_id} {type_id} for {client_entity} (previously {old_gump_id} {old_type_id} for {old_client}");
        }

        if let Some(old_entity) = self.gump_from_id.insert((client_entity, gump_id, type_id), gump_entity) {
            // Duplicate ID.
            warn!("duplicate gump ID {gump_id} {type_id} for {client_entity}");
            self.id_from_gump.remove(&old_entity);
            commands.entity(old_entity).despawn_recursive();
        } else {
            // New gump.
        }
    }

    pub fn remove(&mut self, gump_entity: Entity) {
        if let Some((client_entity, gump_id, type_id)) = self.id_from_gump.remove(&gump_entity) {
            self.gump_from_id.remove(&(client_entity, gump_id, type_id));
        }
    }
}

#[derive(Debug, Clone, Event)]
pub struct OnClientOpenGump {
    pub client_entity: Entity,
    pub gump: Entity,
}

#[derive(Debug, Clone, Event)]
pub struct OnClientCloseGump {
    pub client_entity: Entity,
    pub gump: Entity,
    pub button_id: u32,
    pub on_switches: SmallVec<[u32; 16]>,
    pub text_fields: Vec<String>,
}

pub fn update_gumps(
    mut commands: Commands,
    mut gumps: ResMut<GumpLookup>,
    mut clients: Query<(&NetClient, &mut GumpIdAllocator)>,
    new_gumps: Query<
        (Entity, &Gump, &GumpClient),
        Without<GumpId>,
    >,
    updated_gumps: Query<
        (Entity, &Gump, &GumpClient, Ref<GumpId>),
        Without<GumpSent>,
    >,
    mut removed_gumps: RemovedComponents<GumpId>,
) {
    for (entity, gump, client_entity) in &new_gumps {
        let Ok((client, mut allocator)) = clients.get_mut(**client_entity) else {
            continue;
        };

        let id = allocator.allocate();
        gumps.insert(&mut commands, entity, **client_entity, *id, gump.type_id);
        commands.entity(entity)
            .insert((
                id,
                GumpSent,
            ));
        client.send_packet(gump.to_packet(*id));
    }

    for (entity, gump, client, id) in &updated_gumps {
        let Ok((client, _)) = clients.get(**client) else {
            continue;
        };

        commands.entity(entity)
            .insert(GumpSent);
        client.send_packet(gump.to_packet(**id));
    }

    for entity in removed_gumps.read() {
        gumps.remove(entity);
    }
}

pub fn plugin(app: &mut App) {
    app
        .register_type::<Gump>()
        .register_type::<GumpClient>()
        .register_type::<GumpId>()
        .register_type::<GumpSent>()
        .register_type::<GumpIdAllocator>()
        .register_type::<GumpLookup>()
        .init_resource::<GumpLookup>()
        .add_event::<OnClientCloseGump>()
        .add_systems(Last, (
            update_gumps.in_set(ServerSet::Send),
        ));
}
