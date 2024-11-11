use std::marker::PhantomData;

use bevy::ecs::archetype::{ArchetypeId, Archetypes};
use bevy::ecs::component::ComponentId;
use bevy::ecs::entity::Entities;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::utils::HashMap;

use crate::DefaultGameSet;

pub trait EntityEvent: Event {
    fn target(&self) -> Entity;
}

#[derive(Clone, Debug, Resource)]
pub struct EntityEvents<E: EntityEvent> {
    events: Vec<E>,
}

impl<E: EntityEvent> Default for EntityEvents<E> {
    fn default() -> Self {
        EntityEvents {
            events: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Component)]
pub struct EntityEventRoute<E: EntityEvent> {
    required_components: Vec<ComponentId>,
    _marker: PhantomData<E>,
}

impl<E: EntityEvent> EntityEventRoute<E> {
    pub fn for_bundle<B: Bundle>(world: &mut World) -> EntityEventRoute<E> {
        let mut required_components = Vec::new();

        world.register_bundle::<B>();
        B::get_component_ids(world.components(), &mut |id| {
            required_components.push(id.expect("missing bundle component ID"));
        });

        EntityEventRoute {
            required_components,
            _marker: PhantomData,
        }
    }
}

#[derive(Clone, Debug, Component)]
pub struct EntityEventQueue<E: EntityEvent> {
    pending_events: Vec<usize>,
    _marker: PhantomData<E>,
}

impl<E: EntityEvent> Default for EntityEventQueue<E> {
    fn default() -> Self {
        EntityEventQueue {
            pending_events: Vec::new(),
            _marker: PhantomData,
        }
    }
}

#[derive(Clone, Debug, Component)]
pub struct EntityEventRouteMarker<E: EntityEvent, B: Bundle> {
    _marker: PhantomData<(E, B)>,
}

impl<E: EntityEvent, B: Bundle> Default for EntityEventRouteMarker<E, B> {
    fn default() -> Self {
        EntityEventRouteMarker {
            _marker: PhantomData,
        }
    }
}

#[derive(SystemParam)]
pub struct EntityEventReader<'w, E: EntityEvent, B: Bundle> {
    events: ResMut<'w, EntityEvents<E>>,
    route: Single<'w, &'static EntityEventQueue<E>, With<EntityEventRouteMarker<E, B>>>,
}

impl<'w, E: EntityEvent, B: Bundle> EntityEventReader<'w, E, B> {
    pub fn iter_indices(&self) -> impl Iterator<Item = usize> + 'w {
        let pending_events = self.route.pending_events.as_slice();
        pending_events.iter().copied()
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut E> {
        self.events.events.get_mut(index)
    }

    pub fn read(&mut self) -> impl Iterator<Item = &mut E> + 'w {
        let pending_events = self.route.pending_events.as_slice();
        let events = self.events.events.as_mut_ptr();

        pending_events.iter()
            .copied()
            .map(move |index| {
                // SAFETY: pending_events never includes duplicates.
                unsafe { events.add(index).as_mut().unwrap() }
            })
    }
}

#[derive(Clone, Debug, Default, Resource)]
pub struct EntityEventRouter<E: EntityEvent> {
    _marker: PhantomData<E>,
}

pub fn dispatch_events<E: EntityEvent>(
    entities: &Entities,
    archetypes: &Archetypes,
    mut routes: Query<(Entity, Ref<EntityEventRoute<E>>, &mut EntityEventQueue<E>)>,
    mut removed_routes: RemovedComponents<EntityEventRoute<E>>,
    mut in_events: ResMut<Events<E>>,
    mut out_events: ResMut<EntityEvents<E>>,
    mut archetype_cache: Local<HashMap<ArchetypeId, Vec<Entity>>>,
) {
    let routes_changed = !removed_routes.is_empty() ||
        routes.iter().any(|(_, r, _)| r.is_changed());
    removed_routes.clear();

    if routes_changed {
        archetype_cache.clear();
    }

    for (_, _, mut queue) in &mut routes {
        queue.pending_events.clear();
    }

    out_events.events.clear();
    for event in in_events.drain() {
        let target = event.target();
        let Some(location) = entities.get(target) else {
            warn!("Unable to dispatch event for destroyed entity {target}");
            continue;
        };

        let hit_routes = archetype_cache
            .entry(location.archetype_id)
            .or_insert_with(|| {
                let archetype = archetypes.get(location.archetype_id).unwrap();
                let mut out_routes = Vec::new();

                for (entity, route, _) in &routes {
                    if route.required_components.iter().all(|id| archetype.contains(*id)) {
                        out_routes.push(entity);
                    }
                }

                out_routes
            });

        let offset = out_events.events.len();
        for route_entity in hit_routes.iter() {
            let (_, _, mut queue) = routes.get_mut(*route_entity).unwrap();
            queue.pending_events.push(offset);
        }

        out_events.events.push(event);
    }
}

#[derive(Clone, Debug)]
pub struct EntityEventPlugin<E: EntityEvent>(PhantomData<E>);

impl<E: EntityEvent> Default for EntityEventPlugin<E> {
    fn default() -> Self {
        EntityEventPlugin(PhantomData)
    }
}

impl<E: EntityEvent> Plugin for EntityEventPlugin<E> {
    fn build(&self, app: &mut App) {
        app
            .add_event::<E>()
            .init_resource::<EntityEvents<E>>()
            .add_systems(First, (
                dispatch_events::<E>.in_set(DefaultGameSet::DispatchEvents),
            ));
    }
}

#[derive(Clone, Debug)]
pub struct EntityEventRoutePlugin<E: EntityEvent, B: Bundle>(PhantomData<(E, B)>);

impl<E: EntityEvent, B: Bundle> Default for EntityEventRoutePlugin<E, B> {
    fn default() -> Self {
        EntityEventRoutePlugin(PhantomData)
    }
}

impl<E: EntityEvent, B: Bundle> Plugin for EntityEventRoutePlugin<E, B> {
    fn build(&self, app: &mut App) {
        let query_state = app.world_mut().query_filtered::<(), With<EntityEventRouteMarker<E, B>>>();
        if query_state.is_empty(app.world(), app.world().last_change_tick(), app.world().last_change_tick()) {
            let route = EntityEventRoute::<E>::for_bundle::<B>(app.world_mut());
            app.world_mut().spawn((
                route,
                EntityEventQueue::<E>::default(),
                EntityEventRouteMarker::<E, B>::default(),
            ));
        }
    }
}
