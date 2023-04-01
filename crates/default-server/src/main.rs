use std::collections::VecDeque;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context};
use bevy::ecs::system::CommandQueue;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use clap::Parser;
use futures::future::join;
use log::info;
use serde::Deserialize;
use tokio::fs;
use tokio::net::{lookup_host, TcpListener};
use tokio::sync::mpsc;
use tokio::time::sleep;

use yewoh::assets::multi::load_multi_data;
use yewoh::assets::tiles::load_tile_data;
use yewoh_default_game::data::prefab::{Prefab, PrefabCollection, PrefabCommandsExt, PrefabFactory};
use yewoh_default_game::data::static_data;
use yewoh_default_game::DefaultGamePlugins;
use yewoh_server::async_runtime::AsyncRuntime;
use yewoh_server::game_server::listen_for_game;
use yewoh_server::lobby::{listen_for_lobby, LocalLobby};
use yewoh_server::world::map::{Chunk, create_map_entities, create_statics, MultiDataResource, Static, TileDataResource};
use yewoh_server::world::net::{NetCommandsExt, NetServer};
use yewoh_server::world::ServerPlugin;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Path to the Ultima Online Classic data.
    #[clap(short, long, default_value = "uodata", env = "UO_DATA")]
    uo_data_path: PathBuf,

    /// Path to the Yewoh server data.
    #[clap(short, long, default_value = "data", env = "YEWOH_DATA")]
    data_path: PathBuf,

    /// The display name of this game server.
    #[clap(short, long, default_value = "Yewoh Server", env = "YEWOH_SERVER_NAME")]
    server_display_name: String,

    /// Whether or not to enable packet encryption.
    #[clap(short, long, default_value = "true", env = "YEWOH_ENCRYPTION")]
    encryption: bool,

    /// The external address of this server to provide to clients.
    #[clap(short, long, default_value = "127.0.0.1", env = "YEWOH_ADVERTISE_ADDRESS")]
    advertise_address: String,

    /// The bind address for the HTTP server.
    #[clap(long, default_value = "0.0.0.0:2595", env = "YEWOH_HTTP_BIND")]
    http_bind: String,

    /// The bind address for the lobby server.
    #[clap(long, default_value = "0.0.0.0:2593", env = "YEWOH_LOBBY_BIND")]
    lobby_bind: String,

    /// The bind address for the game server.
    #[clap(long, default_value = "0.0.0.0:2594", env = "YEWOH_GAME_BIND")]
    game_bind: String,
}

fn main() -> anyhow::Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let _guard = rt.enter();

    let args = Args::parse();
    let mut app = App::new();
    app
        .add_plugins(MinimalPlugins)
        .add_plugin(LogPlugin::default())
        .add_plugin(ServerPlugin)
        .add_plugins(DefaultGamePlugins);

    let static_data = rt.block_on(static_data::load_from_directory(&args.data_path))?;
    let map_infos = static_data.maps.map_infos();
    let tile_data = rt.block_on(load_tile_data(&args.uo_data_path))?;
    let multi_data = rt.block_on(load_multi_data(&args.uo_data_path))?;

    // Load UO data
    info!("Loading map data...");
    rt.block_on(create_map_entities(&mut app.world, &map_infos, &args.uo_data_path))?;
    info!("Loading statics...");
    rt.block_on(create_statics(&mut app.world, &map_infos, &tile_data, &args.uo_data_path))?;

    // Load server data
    let mut prefabs = PrefabCollection::default();
    rt.block_on(prefabs.load_from_directory(&app.world.resource(), &args.data_path.join("prefabs")))?;
    info!("Loaded {} prefabs", prefabs.len());
    app.insert_resource(prefabs);

    // Spawn map data
    let mut query = app.world.query_filtered::<(), With<Chunk>>();
    info!("Spawned {} map chunks", query.iter(&app.world).count());
    let mut query = app.world.query_filtered::<(), With<Static>>();
    info!("Spawned {} statics", query.iter(&app.world).count());
    rt.block_on(load_static_entities(&mut app.world, &args.data_path.join("entities")))?;

    let external_ip = rt.block_on(lookup_host(format!("{}:0", &args.advertise_address)))?
        .filter_map(|entry| match entry {
            SocketAddr::V4(v4) => Some(*v4.ip()),
            _ => None,
        })
        .next()
        .ok_or_else(|| anyhow!("couldn't resolve {}", &args.advertise_address))?;

    let game_port = SocketAddr::from_str(&args.game_bind)?.port();
    let (new_session_requests_tx, new_session_requests) = mpsc::unbounded_channel();
    let lobby = LocalLobby::new(
        args.server_display_name, external_ip, game_port, 0, new_session_requests_tx);

    let (lobby_listener, game_listener) = rt.block_on(join(
        TcpListener::bind(&args.lobby_bind),
        TcpListener::bind(&args.game_bind),
    ));

    let lobby_listener = lobby_listener?;
    let game_listener = game_listener?;

    let lobby_handle = rt.spawn(listen_for_lobby(lobby_listener, args.encryption, move || lobby.clone()));

    let (new_session_tx, new_session_rx) = mpsc::unbounded_channel();
    let game_handle = rt.spawn(listen_for_game(game_listener, new_session_tx));

    let http_app = axum::Router::new();
    let http_server_handle = rt.spawn(axum::Server::bind(&SocketAddr::from_str(&args.http_bind)?)
        .serve(http_app.into_make_service()));

    app
        .insert_resource(AsyncRuntime::from(rt.handle().clone()))
        .insert_resource(NetServer::new(args.encryption, new_session_requests, new_session_rx))
        .insert_resource(map_infos)
        .insert_resource(static_data)
        .insert_resource(TileDataResource { tile_data })
        .insert_resource(MultiDataResource { multi_data });

    info!("Listening for http connections on {}", &args.http_bind);
    info!("Listening for lobby connections on {}", &args.lobby_bind);
    info!("Listening for game connections on {}", &args.game_bind);

    let frame_wait = Duration::from_millis(20);
    loop {
        let start_time = Instant::now();
        app.update();

        if game_handle.is_finished() {
            rt.block_on(game_handle)??;
            return Err(anyhow!("failed to serve game connections"));
        }

        if lobby_handle.is_finished() {
            rt.block_on(lobby_handle)??;
            return Err(anyhow!("failed to serve lobby"));
        }

        if http_server_handle.is_finished() {
            rt.block_on(http_server_handle)??;
            return Err(anyhow!("failed to serve http API"));
        }

        let end_time = Instant::now();
        let frame_duration = end_time - start_time;
        if frame_duration < frame_wait {
            rt.block_on(sleep(frame_wait - frame_duration));
        }
    }
}

async fn load_static_entities(world: &mut World, path: &Path) -> anyhow::Result<()> {
    let mut to_visit = VecDeque::new();
    to_visit.push_back(path.to_path_buf());

    let factory = world.resource::<PrefabFactory>();
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new_from_entities(&mut queue, world.entities());
    let mut count = 0;

    while let Some(next) = to_visit.pop_front() {
        let mut entries = fs::read_dir(&next).await?;
        while let Some(entry) = entries.next_entry().await? {
            let metadata = entry.metadata().await?;
            if metadata.is_dir() {
                to_visit.push_back(next.join(entry.file_name()));
            } else if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".yaml") {
                    let full_path = next.join(entry.file_name());
                    let contents = fs::read_to_string(&full_path).await?;
                    let items = serde_yaml::from_str::<Vec<serde_yaml::Value>>(&contents)?;
                    for item in items {
                        let prefab = factory.with(|| Prefab::deserialize(item))
                            .with_context(|| format!("spawning {:?}", full_path))?;
                        commands.spawn_empty()
                            .insert_prefab(Arc::new(prefab))
                            .assign_network_id();
                        count += 1;
                    }
                }
            }
        }
    }

    queue.apply(world);
    log::info!("Spawned {count} entities");
    Ok(())
}
