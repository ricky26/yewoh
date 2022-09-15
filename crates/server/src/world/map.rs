use std::path::Path;

use bevy_ecs::prelude::*;
use bevy_ecs::system::CommandQueue;
use futures::future::{self, TryFutureExt};
use glam::IVec3;
use tokio::spawn;
use tokio::sync::mpsc;

use yewoh::assets::map::{CHUNK_SIZE, load_map, load_statics, MapChunk};
use yewoh::assets::tiles::{TileData, TileFlags};
use yewoh::Direction;

use crate::world::entity::{Graphic, MapPosition};
use crate::world::net::MapInfos;

#[derive(Debug, Clone, Default, Component)]
pub struct Chunk {
    pub map_chunk: MapChunk,
}

#[derive(Debug, Clone, Default, Component)]
pub struct Static;

#[derive(Debug, Clone, Default, Component)]
pub struct Surface {
    pub offset: u8,
}

pub fn spawn_chunk(commands: &mut Commands, map_id: u8, chunk_x: usize, chunk_y: usize, map_chunk: MapChunk) {
    let x = (chunk_x * CHUNK_SIZE) as i32;
    let y = (chunk_y * CHUNK_SIZE) as i32;
    let position = IVec3::new(x, y, 0);

    commands.spawn()
        .insert(MapPosition { map_id, position, direction: Direction::default() })
        .insert(Chunk { map_chunk });
}

pub async fn create_map_entities(world: &mut World, map_infos: &MapInfos, uo_data_path: &Path) -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::channel(128);

    let tasks = map_infos.maps.iter()
        .filter(|(_, m)| !m.is_virtual)
        .map(|(map_id, info)| {
            let map_id = *map_id;
            let width = info.size.x as usize;
            let height = info.size.y as usize;
            let tx = tx.clone();
            let uo_data_path = uo_data_path.to_path_buf();
            spawn(async move {
                load_map(&uo_data_path, map_id as usize, width, height, |x, y, chunk| {
                    tx.send((map_id, x, y, chunk)).map_err(Into::into)
                }).await
            })
        })
        .collect::<Vec<_>>();
    drop(tx);

    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, &world);

    while let Some((map_id, x, y, chunk)) = rx.recv().await {
        spawn_chunk(&mut commands, map_id, x, y, chunk);
    }

    future::try_join_all(tasks).await?;
    queue.apply(world);
    Ok(())
}

pub async fn create_statics(
    world: &mut World,
    map_infos: &MapInfos,
    tile_data: &TileData,
    uo_data_path: &Path,
) -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::channel(128);

    let tasks = map_infos.maps.iter()
        .filter(|(_, m)| !m.is_virtual)
        .map(|(map_id, info)| {
            let map_id = *map_id;
            let width = info.size.x as usize;
            let height = info.size.y as usize;
            let tx = tx.clone();
            let uo_data_path = uo_data_path.to_path_buf();
            spawn(async move {
                load_statics(&uo_data_path, map_id as usize, width, height, |s| {
                    tx.send((map_id, s)).map_err(Into::into)
                }).await
            })
        })
        .collect::<Vec<_>>();
    drop(tx);

    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, &world);

    while let Some((map_id, s)) = rx.recv().await {
        let mut entity = commands.spawn();
        entity
            .insert(MapPosition { map_id, position: s.position, direction: Direction::default() })
            .insert(Graphic { id: s.graphic_id, hue: s.hue })
            .insert(Static);

        if let Some(info) = tile_data.items.get(s.graphic_id as usize) {
            if info.flags.contains(TileFlags::SURFACE) {
                entity.insert(Surface { offset: info.height });
            }
        }
    }

    future::try_join_all(tasks).await?;
    queue.apply(world);
    Ok(())
}
