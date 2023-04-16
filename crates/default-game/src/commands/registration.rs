use std::any::TypeId;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;

use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_ecs::system::{Resource, SystemParam};
use clap::Parser;

use yewoh::protocol::{MessageKind, UnicodeTextMessage};
use yewoh_server::world::net::NetClient;

pub trait TextCommand: Parser + Resource {
    fn aliases() -> &'static [&'static str];
}

struct QueueDrain<T> {
    ptr: *mut T,
    index: usize,
    length: usize,
}

impl<T> Drop for QueueDrain<T> {
    fn drop(&mut self) {
        unsafe {
            for i in self.index..self.length {
                std::ptr::read(self.ptr.add(i));
            }
        }
    }
}

impl<T> Iterator for QueueDrain<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.length {
            let value = unsafe {
                std::ptr::read(self.ptr.add(self.index))
            };
            self.index += 1;
            Some(value)
        } else {
            None
        }
    }
}

struct TextCommandQueueStorage {
    ptr: usize,
    length: usize,
    capacity: usize,
    drop: fn(&mut TextCommandQueueStorage),
}

impl TextCommandQueueStorage {
    pub fn new<T>() -> TextCommandQueueStorage {
        let mut vec = ManuallyDrop::new(Vec::<T>::new());
        let length = vec.len();
        let capacity = vec.capacity();
        let ptr = vec.as_mut_ptr() as usize;
        let drop = |me: &mut TextCommandQueueStorage| {
            let ptr = me.ptr as *mut T;
            unsafe {
                Vec::from_raw_parts(ptr, me.length, me.capacity);
            }
        };
        Self { ptr, length, capacity, drop }
    }

    pub unsafe fn push<T>(&mut self, value: T) {
        unsafe {
            let ptr = self.ptr as *mut T;
            let mut vec = ManuallyDrop::new(Vec::from_raw_parts(ptr, self.length, self.capacity));
            vec.push(value);
            self.length = vec.len();
            self.capacity = vec.capacity();
            self.ptr = vec.as_mut_ptr() as usize;
        }
    }

    pub unsafe fn drain<T>(&mut self) -> QueueDrain<T> {
        let length = self.length;
        self.length = 0;
        QueueDrain {
            ptr: self.ptr as *mut T,
            index: 0,
            length,
        }
    }
}

impl Drop for TextCommandQueueStorage {
    fn drop(&mut self) {
        (self.drop)(self)
    }
}

struct Registration {
    enqueue: fn(&mut TextCommandQueueStorage, &NetClient, Entity, &[String]),
    queue: UnsafeCell<TextCommandQueueStorage>,
}

unsafe impl Sync for Registration {}

#[derive(Default, Resource)]
pub struct TextCommands {
    start_character: char,
    commands: HashMap<TypeId, Registration>,
    aliases: HashMap<String, TypeId>,
}

impl TextCommands {
    pub fn new(start_character: char) -> TextCommands {
        Self {
            start_character,
            commands: HashMap::new(),
            aliases: HashMap::new(),
        }
    }

    pub fn is_command(&self, name: &str) -> bool {
        self.aliases.contains_key(name)
    }

    pub fn register<T: TextCommand>(&mut self) {
        let type_id = TypeId::of::<T>();
        self.commands.insert(type_id, Registration {
            enqueue: TextCommandExecutor::enqueue::<T>,
            queue: UnsafeCell::new(TextCommandQueueStorage::new::<(Entity, T)>()),
        });

        for alias in T::aliases() {
            self.aliases.insert(alias.to_string(), type_id);
        }
    }
}

#[derive(SystemParam)]
pub struct TextCommandExecutor<'w, 's> {
    clients: Query<'w, 's, &'static NetClient>,
    commands: ResMut<'w, TextCommands>,
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

        if let Some((type_id, client)) = self.commands.aliases.get(&args[0]).cloned()
            .zip(self.clients.get(from).ok()) {
            let registration = self.commands.commands.get_mut(&type_id).unwrap();
            let queue = unsafe { &mut *registration.queue.get() };
            (registration.enqueue)(queue, client, from, &args);
            true
        } else {
            false
        }
    }

    fn enqueue<T: TextCommand>(
        queue: &mut TextCommandQueueStorage, client: &NetClient, from: Entity, args: &[String],
    ) {
        match T::try_parse_from(args.iter()) {
            Ok(instance) => {
                unsafe { queue.push((from, instance)) };
            }
            Err(err) => {
                client.send_packet(UnicodeTextMessage {
                    entity_id: None,
                    kind: MessageKind::System,
                    language: Default::default(),
                    text: err.to_string(),
                    name: Default::default(),
                    hue: 2751,
                    font: 1,
                    ..Default::default()
                }.into());
            }
        }
    }
}

#[derive(Resource)]
struct TextCommandQueueImpl<T>(PhantomData<T>);

impl<T> Default for TextCommandQueueImpl<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

#[derive(SystemParam)]
pub struct TextCommandQueue<'w, T: Send + Sync + 'static> {
    _lock: ResMut<'w, TextCommandQueueImpl<T>>,
    commands: Res<'w, TextCommands>,
}

impl<'a, T: Resource> TextCommandQueue<'a, T> {
    pub fn iter(&mut self) -> impl Iterator<Item=(Entity, T)> + '_ {
        let registration = self.commands.commands.get(&TypeId::of::<T>())
            .expect("tried to execute unregistered text command");
        unsafe {
            (*registration.queue.get()).drain()
        }
    }
}

pub trait TextCommandRegistrationExt {
    fn add_text_command<T: TextCommand>(&mut self) -> &mut Self;
}

impl TextCommandRegistrationExt for World {
    fn add_text_command<T: TextCommand>(&mut self) -> &mut Self {
        self.init_resource::<TextCommands>();
        self.init_resource::<TextCommandQueueImpl<T>>();
        self.resource_mut::<TextCommands>().register::<T>();
        self
    }
}

impl TextCommandRegistrationExt for App {
    fn add_text_command<T: TextCommand>(&mut self) -> &mut Self {
        self.world.add_text_command::<T>();
        self
    }
}
