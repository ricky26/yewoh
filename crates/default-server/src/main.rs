use std::collections::VecDeque;
use std::io::Cursor;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::anyhow;
use bevy::asset::{handle_internal_asset_events, AssetPath, LoadState};
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::tasks::block_on;
use bevy::time::Time;
use clap::Parser;
use futures::stream::FuturesUnordered;
use futures::{FutureExt, StreamExt, TryFutureExt};
use tokio::fs;
use tokio::net::{lookup_host, TcpListener};
use tokio::sync::mpsc;
use tracing::info;

use yewoh::assets::multi::load_multi_data;
use yewoh::assets::tiles::load_tile_data;
use yewoh_default_game::data::static_data;
use yewoh_default_game::persistence::{migrate, SerializationWorldExt, SerializedBuffers};
use yewoh_default_game::DefaultGamePlugins;
use yewoh_server::async_runtime::AsyncRuntime;
use yewoh_server::game_server::listen_for_game;
use yewoh_server::lobby::{listen_for_lobby, LocalServerRepository};
use yewoh_server::world::connection::NetServer;
use yewoh_server::world::map::{self, Chunk, MultiDataResource, Static, TileDataResource};
use yewoh_server::world::ServerPlugin;

use bevy_fabricator::hot_reload::{FabricatorChanged, WatchForFabricatorChanges};
use bevy_fabricator::{empty_reflect, Fabricate, FabricateExt, Fabricator};
use sqlx::postgres::PgPool;
use yewoh_default_game::accounts::sql::{SqlAccountRepository, SqlAccountRepositoryConfig};
use yewoh_default_game::data::prefabs::PrefabLibrary;
use yewoh_default_game::persistence::db::WorldRepository;
use yewoh_server::world::delta_grid::DeltaGrid;
use yewoh_server::world::spatial::{ChunkLookup, SpatialCharacterLookup, SpatialDynamicItemLookup, SpatialStaticItemLookup};

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

    /// The bind address for unencrypted lobby connections.
    #[clap(long, env = "YEWOH_PLAIN_LOBBY_BIND")]
    plain_lobby_bind: Option<String>,

    /// The address of the database.
    #[clap(long, default_value = "postgres://postgres:postgres@localhost/yewoh", env = "YEWOH_POSTGRES")]
    postgres: String,

    /// The shard ID of this server.
    #[clap(long, default_value = "default", env = "YEWOH_SHARD_ID")]
    shard_id: String,

    #[clap(long, default_value = "false", env = "YEWOH_AUTO_CREATE_ACCOUNTS")]
    auto_create_accounts: bool,
}

fn main() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let _runtime = runtime.enter();

    let frame_wait = Duration::from_millis(20);
    let load_wait = Duration::from_millis(100);
    let args = Args::parse();
    let pool = block_on(async move {
        let pool = Arc::new(PgPool::connect(&args.postgres).await?);
        migrate(&pool).await?;
        Ok::<_, anyhow::Error>(pool)
    })?;

    let external_ip = block_on(lookup_host(format!("{}:0", &args.advertise_address)))?
        .filter_map(|entry| match entry {
            SocketAddr::V4(v4) => Some(*v4.ip()),
            _ => None,
        })
        .next()
        .ok_or_else(|| anyhow!("couldn't resolve {}", &args.advertise_address))?;
    let game_port = SocketAddr::from_str(&args.game_bind)?.port();
    let (new_session_requests_tx, new_session_requests) = mpsc::unbounded_channel();

    let server_repo = LocalServerRepository::new(
        args.server_display_name.clone(), external_ip, game_port, 0, new_session_requests_tx.clone());
    let accounts_repo = SqlAccountRepository::new(SqlAccountRepositoryConfig {
        auto_create_accounts: args.auto_create_accounts,
    }, pool.clone());
    let world_repo = WorldRepository::new(pool.clone(), args.shard_id.clone());

    let abs_data_path = std::fs::canonicalize(&args.data_path)?;

    let mut app = App::new();
    app
        .add_plugins((
            MinimalPlugins,
            LogPlugin::default(),
            AssetPlugin {
                file_path: abs_data_path.to_string_lossy().to_string(),
                ..default()
            },
            DefaultGamePlugins,
            ServerPlugin,
        ));
    app.finish();
    app.cleanup();

    // Load prefabs
    info!("Loading prefabs...");
    let (prefabs, prefab_handles) = load_prefabs(&mut app, load_wait, &args.data_path, "prefabs")?;
    app
        .insert_resource(prefabs)
        .insert_resource(prefab_handles);

    let (static_data, map_infos, tile_data, multi_data, map_entities, static_entities) = block_on(async {
        let static_data = static_data::load_from_directory(&args.data_path).await?;
        let map_infos = static_data.maps.map_infos();
        let tile_data = load_tile_data(&args.uo_data_path).await?;
        let multi_data = load_multi_data(&args.uo_data_path).await?;

        // Load UO data
        info!("Loading map data...");
        let map_entities = map::load_map_entities(&map_infos, &args.uo_data_path).await?;
        info!("Loading statics...");
        let static_entities = map::load_static_entities(&map_infos, &args.uo_data_path).await?;

        Ok::<_, anyhow::Error>((static_data, map_infos, tile_data, multi_data, map_entities, static_entities))
    })?;

    // Spawn map
    info!("Spawning map...");
    map::spawn_map_entities(app.world_mut(), map_entities.into_iter());
    info!("Spawning statics...");
    map::spawn_static_entities(app.world_mut(), &tile_data, &static_entities);

    // Spawn map data
    {
        let mut query = app.world_mut().query_filtered::<(), With<Chunk>>();
        info!("Spawned {} map chunks", query.iter(app.world()).count());
        let mut query = app.world_mut().query_filtered::<(), With<Static>>();
        info!("Spawned {} statics", query.iter(app.world()).count());
    }

    // Initialise spatial lookups
    app
        .insert_resource(SpatialCharacterLookup::new(&map_infos))
        .insert_resource(SpatialDynamicItemLookup::new(&map_infos))
        .insert_resource(SpatialStaticItemLookup::new(&map_infos))
        .insert_resource(ChunkLookup::new(&map_infos))
        .insert_resource(DeltaGrid::new(&map_infos));

    // Spawn static entities
    load_static_entities(&mut app, load_wait, &args.data_path, "entities")?;

    let mut listen_futures = FuturesUnordered::new();
    let (new_session_tx, new_session_rx) = mpsc::unbounded_channel();

    // Listen for game traffic
    {
        let game_listener = block_on(TcpListener::bind(&args.game_bind))?;
        let game_handle = tokio::spawn(listen_for_game(game_listener, new_session_tx.clone()));
        listen_futures.push(async move {
            game_handle.await??;
            return Err(anyhow!("failed to serve game"));
        }.boxed());
    }

    // Listen for encrypted lobby traffic
    {
        let server_repo = server_repo.clone();
        let lobby_listener = block_on(TcpListener::bind(&args.lobby_bind))?;
        let accounts_repo_clone = accounts_repo.clone();
        let lobby_handle = tokio::spawn(listen_for_lobby(
            lobby_listener, true,
            move || server_repo.clone(), move || accounts_repo_clone.clone()));
        listen_futures.push(async move {
            lobby_handle.await??;
            return Err(anyhow!("failed to serve lobby"));
        }.boxed());
    }

    // Listen for unencrypted lobby traffic
    if let Some(lobby_bind) = args.plain_lobby_bind.as_ref() {
        let lobby_listener = block_on(TcpListener::bind(&lobby_bind))?;
        let accounts_repo_clone = accounts_repo.clone();
        let lobby_handle = tokio::spawn(listen_for_lobby(
            lobby_listener, false,
            move || server_repo.clone(), move || accounts_repo_clone.clone()));
        listen_futures.push(async move {
            lobby_handle.await??;
            return Err(anyhow!("failed to serve unencrypted lobby"));
        }.boxed());
    }

    let http_app = axum::Router::new();
    let http_server_handle = tokio::spawn(axum_server::bind(SocketAddr::from_str(&args.http_bind)?)
        .serve(http_app.into_make_service()))
        .map_err(|e| anyhow::Error::from(e))
        .boxed();
    listen_futures.push(http_server_handle);

    app
        .insert_resource(AsyncRuntime::from(tokio::runtime::Handle::current()))
        .insert_resource(NetServer::new(new_session_requests, new_session_rx))
        .insert_resource(map_infos)
        .insert_resource(static_data)
        .insert_resource(TileDataResource { tile_data })
        .insert_resource(MultiDataResource { multi_data })
        .insert_resource(world_repo.clone())
        .insert_resource(accounts_repo.clone())
        .add_systems(Last, (
            scheduled_save,
            update_static_entities,
            update_prefabs,
        ));

    // Load previous state
    if let Some(contents) = block_on(world_repo.get_snapshot())? {
        let mut d = serde_json::Deserializer::from_reader(Cursor::new(&contents));
        app.world_mut().deserialize(&mut d)?;
    }

    static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);
    ctrlc::set_handler(|| {
        info!("Shutting down...");
        SHOULD_EXIT.store(true, Ordering::Relaxed);
    }).expect("failed to register shutdown handler");

    info!("Listening for http connections on {}", &args.http_bind);
    info!("Listening for game connections on {}", &args.game_bind);
    info!("Listening for lobby connections on {}", &args.lobby_bind);
    if let Some(lobby_bind) = args.plain_lobby_bind.as_ref() {
        info!("Listening for unencrypted lobby connections on {}", &lobby_bind);
    }

    let serve_handle = tokio::spawn(async move {
        if let Some(result) = listen_futures.next().await {
            result??;
        }
        Ok::<_, anyhow::Error>(())
    });

    loop {
        if SHOULD_EXIT.load(Ordering::Relaxed) {
            let contents = app.world_mut().serialize();
            let repo = app.world().resource::<WorldRepository>();
            block_on(write_save(repo, contents))?;
            info!("Saved snapshot");
            return Ok(());
        }

        let start_time = Instant::now();
        app.update();

        if serve_handle.is_finished() {
            block_on(serve_handle)??;
            return Err(anyhow!("failed to serve services"));
        }

        #[cfg(feature = "trace_tracy")]
        tracing::event!(tracing::Level::INFO, message = "finished frame", tracy.frame_mark = true);

        let _span = info_span!("frame sleep").entered();
        let end_time = Instant::now();
        let frame_duration = end_time - start_time;
        if frame_duration < frame_wait {
            std::thread::sleep(frame_wait - frame_duration);
        }
    }
}

#[derive(Resource)]
#[allow(dead_code)]
struct PrefabHandles(Vec<Handle<Fabricator>>);

fn load_prefabs(
    app: &mut App,
    step_duration: Duration,
    root_path: &Path,
    start_path: impl Into<PathBuf>,
) -> anyhow::Result<(PrefabLibrary, PrefabHandles)> {
    let mut to_visit = VecDeque::new();
    to_visit.push_back(start_path.into());

    let asset_server = app.world().resource::<AssetServer>().clone();
    let queue = block_on(async {
        let mut queue = Vec::new();

        while let Some(dir_path) = to_visit.pop_front() {
            let abs_dir_path = root_path.join(&dir_path);

            let mut entries = fs::read_dir(&abs_dir_path).await?;
            while let Some(entry) = entries.next_entry().await? {
                let metadata = entry.metadata().await?;
                if metadata.is_dir() {
                    to_visit.push_back(dir_path.join(entry.file_name()));
                } else if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".fab") {
                        let full_path = dir_path.join(entry.file_name());
                        let name = full_path.file_stem().unwrap().to_string_lossy().to_string();
                        let asset_path = AssetPath::from(full_path);
                        let handle = asset_server.load(asset_path);
                        queue.push((name, handle));
                    }
                }
            }
        }

        Ok::<_, anyhow::Error>(queue)
    })?;

    let mut handles = Vec::new();
    let mut library = PrefabLibrary::default();
    for (name, handle) in queue {
        handles.push(handle.clone());

        loop {
            match asset_server.get_load_state(&handle).unwrap() {
                LoadState::NotLoaded => unreachable!(),
                LoadState::Loading => {
                    std::thread::sleep(step_duration);
                    handle_internal_asset_events(app.world_mut());
                    continue;
                }
                LoadState::Loaded => break,
                LoadState::Failed(err) => {
                    warn!("failed to load asset: {err}");
                    break;
                }
            }
        }

        let fabricators = app.world().resource::<Assets<Fabricator>>();
        if let Some(fabricator) = fabricators.get(&handle) {
            library.insert(name, fabricator.clone());
        }
    }

    info!("Loaded {} prefabs", library.len());
    Ok((library, PrefabHandles(handles)))
}

fn update_prefabs(
    mut events: EventReader<AssetEvent<Fabricator>>,
    mut prefabs: ResMut<PrefabLibrary>,
    asset_server: Res<AssetServer>,
    fabricators: Res<Assets<Fabricator>>,
) {
    let update_prefab = |prefabs: &mut ResMut<PrefabLibrary>, asset_server: &AssetServer, fabricators: &Assets<Fabricator>, id: AssetId<Fabricator>| {
        let Some(fabricator) = fabricators.get(id) else {
            return;
        };

        let Some(path) = asset_server.get_path(id) else {
            return;
        };

        if !path.path().starts_with("prefabs/") {
            return;
        }

        let name = path.path().file_stem().unwrap().to_string_lossy().to_string();
        info!("Reloaded prefab '{name}'");
        prefabs.insert(name, fabricator.clone());
    };

    for event in events.read() {
        match event {
            AssetEvent::LoadedWithDependencies { id } => {
                update_prefab(&mut prefabs, asset_server.as_ref(), fabricators.as_ref(), *id);
            }
            AssetEvent::Modified { id } => {
                update_prefab(&mut prefabs, asset_server.as_ref(), fabricators.as_ref(), *id);
            }
            _ => {}
        }
    }
}

#[derive(Component)]
struct StaticEntity(AssetPath<'static>);

fn load_static_entities(
    app: &mut App,
    step_duration: Duration,
    root_path: &Path,
    entities_path: impl Into<PathBuf>,
) -> anyhow::Result<()> {
    let mut to_visit = VecDeque::new();
    to_visit.push_back(entities_path.into());

    let asset_server = app.world().resource::<AssetServer>().clone();
    let fab_queue = block_on(async {
        let mut fab_queue = Vec::new();

        while let Some(dir_path) = to_visit.pop_front() {
            let abs_dir_path = root_path.join(&dir_path);

            let mut entries = fs::read_dir(&abs_dir_path).await?;
            while let Some(entry) = entries.next_entry().await? {
                let metadata = entry.metadata().await?;
                if metadata.is_dir() {
                    to_visit.push_back(dir_path.join(entry.file_name()));
                } else if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".fab") {
                        let full_path = dir_path.join(entry.file_name());
                        let name = full_path.file_stem().unwrap().to_string_lossy().to_string();
                        let asset_path = AssetPath::from(full_path);
                        let template = asset_server.load(&asset_path);
                        fab_queue.push((template, name, asset_path));
                    }
                }
            }
        }

        Ok::<_, anyhow::Error>(fab_queue)
    })?;

    let mut count = 0;
    for (template, name, asset_path) in fab_queue {
        loop {
            match asset_server.get_load_state(&template).unwrap() {
                LoadState::NotLoaded => unreachable!(),
                LoadState::Loading => {
                    std::thread::sleep(step_duration);
                    handle_internal_asset_events(app.world_mut());
                    continue;
                }
                LoadState::Loaded => break,
                LoadState::Failed(err) => {
                    error!("failed to load static entity: {err}");
                    break;
                }
            }
        }

        let fabricators = app.world().resource::<Assets<Fabricator>>();
        let parameters = empty_reflect();
        let fabricate = Fabricate {
            fabricator: template,
            parameters: parameters.clone(),
        };

        let request = fabricate.to_request(fabricators, Some(&asset_server));
        let mut entity = app.world_mut()
            .spawn((
                Name::new(name),
                StaticEntity(asset_path),
                fabricate,
                WatchForFabricatorChanges,
            ));
        match request {
            Ok(Some(request)) => {
                entity.fabricate(request);
            }
            Ok(None) => {}
            Err(err) => error!("failed to spawn static entity: {err}"),
        }
        count += 1;
    }

    info!("Spawned {count} entities");
    Ok(())
}

fn update_static_entities(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    fabricators: Res<Assets<Fabricator>>,
    old_entities: Query<(Entity, &Name, &Fabricate, &StaticEntity), With<FabricatorChanged>>,
) {
    for (entity, name, fabricate, path) in &old_entities {
        let fabricate = fabricate.clone();
        let asset_path = path.0.clone();
        let request = match fabricate.to_request(&fabricators, Some(&asset_server)) {
            Ok(Some(r)) => r,
            Ok(None) => continue,
            Err(err) => {
                error!("failed to load updated fabricator: {err}");
                continue;
            }
        };

        info!("Reloaded static entity '{name}'");
        commands.entity(entity).despawn_recursive();
        commands
            .spawn((
                name.clone(),
                StaticEntity(asset_path),
                fabricate,
                WatchForFabricatorChanges,
            ))
            .fabricate(request);
    }
}

struct SaveTimer {
    timer: Timer,
}

impl Default for SaveTimer {
    fn default() -> Self {
        Self { timer: Timer::new(Duration::from_secs(30), TimerMode::Repeating) }
    }
}

async fn write_save(repo: &WorldRepository, buffers: SerializedBuffers) -> anyhow::Result<()> {
    let mut output = Vec::new();
    let mut s = serde_json::Serializer::new(&mut output);
    buffers.serialize(&mut s)?;
    repo.put_snapshot(output).await?;
    Ok(())
}

fn scheduled_save(world: &mut World, mut timer: Local<SaveTimer>) {
    if !timer.timer.tick(world.resource::<Time>().delta()).just_finished() {
        return;
    }

    let buffers = world.serialize();
    let repo = world.resource::<WorldRepository>().clone();
    world.resource::<AsyncRuntime>().spawn(async move {
        if let Err(e) = write_save(&repo, buffers).await {
            warn!("failed to save: {e}");
        } else {
            info!("Saved snapshot");
        }
    });
}
