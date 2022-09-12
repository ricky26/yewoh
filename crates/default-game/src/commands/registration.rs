use std::collections::HashMap;
use bevy_ecs::archetype::Archetype;

use bevy_ecs::prelude::*;
use bevy_ecs::system::{ResMutState, Resource, ResState, SystemMeta, SystemParam, SystemParamFetch, SystemParamState};
use clap::Parser;

use yewoh::protocol::{MessageKind, UnicodeTextMessage};
use yewoh_server::world::net::NetClient;

pub trait TextCommand: Parser + Resource {
    fn aliases() -> &'static [&'static str];
}

#[derive(Clone)]
struct Registration {
    exec: fn(&World, &NetClient, Entity, &[String]),
}

#[derive(Clone)]
pub struct TextCommands {
    start_character: char,
    commands: HashMap<String, Registration>,
}

impl TextCommands {
    pub fn new(start_character: char) -> TextCommands {
        Self {
            start_character,
            commands: HashMap::new(),
        }
    }

    pub fn is_command(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }
}

pub struct TextCommandExecutor<'w, 's> {
    world: &'w World,
    clients: Query<'w, 's, &'static NetClient>,
    commands: ResMut<'w, TextCommands>,
}

pub struct TextCommandExecutorState {
    clients: QueryState<&'static NetClient>,
    commands: ResMutState<TextCommands>,
}

impl<'w, 's> TextCommandExecutor<'w, 's> {
    pub fn try_split_exec(&mut self, from: Entity, line: &str) -> bool {
        if !line.starts_with(self.commands.start_character) {
            return false;
        }

        let args = match shell_words::split(&line[1..]) {
            Ok(x) => x,
            Err(_) => {
                return false;
            }
        };
        self.try_exec(from, &args)
    }

    pub fn try_exec(&mut self, from: Entity, args: &[String]) -> bool {
        if args.len() == 0 {
            return false;
        }

        if let Some((registration, client)) = self.commands.commands.get(&args[0])
            .zip(self.clients.get(from).ok()) {
            (registration.exec)(self.world, client, from, &args);
            true
        } else {
            false
        }
    }

    fn exec<T: TextCommand>(world: &World, client: &NetClient, from: Entity, args: &[String]) {
        match T::try_parse_from(args.iter()) {
            Ok(instance) => {
                unsafe {
                    let mut queue = world.get_resource_unchecked_mut::<TextCommandQueueImpl<T>>().unwrap();
                    queue.0.push((from, instance));
                }
            }
            Err(err) => {
                client.send_packet(UnicodeTextMessage {
                    entity_id: None,
                    kind: MessageKind::System,
                    language: "".to_string(),
                    text: err.to_string(),
                    name: "".to_string(),
                    hue: 2751,
                    font: 1,
                    ..Default::default()
                }.into());
            }
        }
    }
}

impl<'w, 's> SystemParam for TextCommandExecutor<'w, 's> {
    type Fetch = TextCommandExecutorState;
}

unsafe impl SystemParamState for TextCommandExecutorState {
    fn init(world: &mut World, system_meta: &mut SystemMeta) -> Self {
        let commands = ResMutState::init(world, system_meta);
        let clients = QueryState::init(world, system_meta);
        Self { commands, clients }
    }

    fn new_archetype(&mut self, archetype: &Archetype, system_meta: &mut SystemMeta) {
        SystemParamState::new_archetype(&mut self.clients, archetype, system_meta);
    }
}

impl<'w, 's> SystemParamFetch<'w, 's> for TextCommandExecutorState {
    type Item = TextCommandExecutor<'w, 's>;

    #[inline]
    unsafe fn get_param(
        state: &'s mut Self,
        system_meta: &SystemMeta,
        world: &'w World,
        change_tick: u32,
    ) -> Self::Item {
        TextCommandExecutor {
            world,
            clients: QueryState::get_param(&mut state.clients, system_meta, world, change_tick),
            commands: ResMutState::get_param(&mut state.commands, system_meta, world, change_tick),
        }
    }
}

struct TextCommandQueueImpl<T: Resource>(pub Vec<(Entity, T)>);

impl<T: Resource> Default for TextCommandQueueImpl<T> {
    fn default() -> Self {
        TextCommandQueueImpl(Vec::new())
    }
}

pub struct TextCommandQueue<'a, T: Resource>(ResMut<'a, TextCommandQueueImpl<T>>);

pub struct TextCommandQueueState<T: Resource>(ResMutState<TextCommandQueueImpl<T>>);

impl<'a, T: Resource> TextCommandQueue<'a, T> {
    pub fn iter(&mut self) -> impl Iterator<Item=(Entity, T)> + '_ {
        self.0.0.drain(..)
    }
}

impl<'a, T: TextCommand> SystemParam for TextCommandQueue<'a, T> {
    type Fetch = TextCommandQueueState<T>;
}

unsafe impl<T: Resource + TextCommand> SystemParamState for TextCommandQueueState<T> {
    fn init(world: &mut World, system_meta: &mut SystemMeta) -> Self {
        world.init_resource::<TextCommandQueueImpl<T>>();
        let mut text_commands = world.resource_mut::<TextCommands>();

        for alias in T::aliases() {
            text_commands.commands.insert(
                alias.to_string(),
                Registration {
                    exec: TextCommandExecutor::exec::<T>,
                });
        }

        ResState::<TextCommands>::init(world, system_meta);
        let res_state = ResMutState::init(world, system_meta);
        Self(res_state)
    }
}

impl<'w, 's, T: TextCommand> SystemParamFetch<'w, 's> for TextCommandQueueState<T> {
    type Item = TextCommandQueue<'w, T>;

    #[inline]
    unsafe fn get_param(
        state: &'s mut Self,
        system_meta: &SystemMeta,
        world: &'w World,
        change_tick: u32,
    ) -> Self::Item {
        TextCommandQueue(ResMutState::get_param(&mut state.0, system_meta, world, change_tick))
    }
}


