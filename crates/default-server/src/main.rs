use std::collections::VecDeque;
use std::io::Cursor;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail};
use bevy::asset::{AssetPath, LoadState};
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::time::Time;
use clap::Parser;
use futures::future::join;
use tokio::fs;
use tokio::net::{lookup_host, TcpListener};
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::info;

use yewoh::assets::multi::load_multi_data;
use yewoh::assets::tiles::load_tile_data;
use yewoh_default_game::data::static_data;
use yewoh_default_game::persistence::{migrate, SerializationWorldExt, SerializedBuffers};
use yewoh_default_game::DefaultGamePlugins;
use yewoh_server::async_runtime::AsyncRuntime;
use yewoh_server::game_server::listen_for_game;
use yewoh_server::lobby::{listen_for_lobby, LocalServerRepository};
use yewoh_server::world::map::{create_map_entities, create_statics, Chunk, MultiDataResource, Static, TileDataResource};
use yewoh_server::world::net::{AssignNetId, NetServer};
use yewoh_server::world::ServerPlugin;

use bevy_fabricator::hot_reload::{FabricatorChanged, WatchForFabricatorChanges};
use bevy_fabricator::{empty_reflect, Fabricate, FabricateExt, Fabricator};
use sqlx::postgres::PgPool;
use yewoh_default_game::accounts::sql::{SqlAccountRepository, SqlAccountRepositoryConfig};
use yewoh_default_game::data::prefabs::PrefabLibrary;
use yewoh_default_game::persistence::db::WorldRepository;

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

    /// The address of the database.
    #[clap(long, default_value = "postgres://postgres:postgres@localhost/yewoh", env = "YEWOH_POSTGRES")]
    postgres: String,

    /// The shard ID of this server.
    #[clap(long, default_value = "default", env = "YEWOH_SHARD_ID")]
    shard_id: String,

    #[clap(long, default_value = "false", env = "YEWOH_AUTO_CREATE_ACCOUNTS")]
    auto_create_accounts: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let pool = Arc::new(PgPool::connect(&args.postgres).await?);
    migrate(&pool).await?;

    let external_ip = lookup_host(format!("{}:0", &args.advertise_address)).await?
        .filter_map(|entry| match entry {
            SocketAddr::V4(v4) => Some(*v4.ip()),
            _ => None,
        })
        .next()
        .ok_or_else(|| anyhow!("couldn't resolve {}", &args.advertise_address))?;
    let game_port = SocketAddr::from_str(&args.game_bind)?.port();
    let (new_session_requests_tx, new_session_requests) = mpsc::unbounded_channel();

    let server_repo = LocalServerRepository::new(
        args.server_display_name, external_ip, game_port, 0, new_session_requests_tx);
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

    let static_data = static_data::load_from_directory(&args.data_path).await?;
    let map_infos = static_data.maps.map_infos();
    let tile_data = load_tile_data(&args.uo_data_path).await?;
    let multi_data = load_multi_data(&args.uo_data_path).await?;

    // Load UO data
    info!("Loading map data...");
    create_map_entities(app.world_mut(), &map_infos, &args.uo_data_path).await?;
    info!("Loading statics...");
    create_statics(app.world_mut(), &map_infos, &tile_data, &args.uo_data_path).await?;

    // Load prefabs
    let prefabs = load_prefabs(&mut app, &args.data_path, "prefabs").await?;

    // Spawn map data
    let mut query = app.world_mut().query_filtered::<(), With<Chunk>>();
    info!("Spawned {} map chunks", query.iter(app.world()).count());
    let mut query = app.world_mut().query_filtered::<(), With<Static>>();
    info!("Spawned {} statics", query.iter(app.world()).count());
    load_static_entities(&mut app, &args.data_path, "entities").await?;

    let (lobby_listener, game_listener) = join(
        TcpListener::bind(&args.lobby_bind),
        TcpListener::bind(&args.game_bind),
    ).await;

    let lobby_listener = lobby_listener?;
    let game_listener = game_listener?;

    let accounts_repo_clone = accounts_repo.clone();
    let lobby_handle = tokio::spawn(listen_for_lobby(
        lobby_listener, args.encryption,
        move || server_repo.clone(), move || accounts_repo_clone.clone()));

    let (new_session_tx, new_session_rx) = mpsc::unbounded_channel();
    let game_handle = tokio::spawn(listen_for_game(game_listener, new_session_tx));

    let http_app = axum::Router::new();
    let http_server_handle = tokio::spawn(axum_server::bind(SocketAddr::from_str(&args.http_bind)?)
        .serve(http_app.into_make_service()));

    app
        .insert_resource(AsyncRuntime::from(Handle::current()))
        .insert_resource(NetServer::new(args.encryption, new_session_requests, new_session_rx))
        .insert_resource(map_infos)
        .insert_resource(static_data)
        .insert_resource(TileDataResource { tile_data })
        .insert_resource(MultiDataResource { multi_data })
        .insert_resource(world_repo.clone())
        .insert_resource(accounts_repo.clone())
        .insert_resource(prefabs)
        .add_systems(Last, (
            scheduled_save,
            update_static_entities,
        ));

    // Load previous state
    if let Some(contents) = world_repo.get_snapshot().await? {
        let mut d = serde_json::Deserializer::from_reader(Cursor::new(&contents));
        app.world_mut().deserialize(&mut d)?;
    }

    static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);
    ctrlc::set_handler(|| {
        info!("Shutting down...");
        SHOULD_EXIT.store(true, Ordering::Relaxed);
    }).expect("failed to register shutdown handler");

    info!("Listening for http connections on {}", &args.http_bind);
    info!("Listening for lobby connections on {}", &args.lobby_bind);
    info!("Listening for game connections on {}", &args.game_bind);

    let frame_wait = Duration::from_millis(20);
    loop {
        if SHOULD_EXIT.load(Ordering::Relaxed) {
            let contents = app.world_mut().serialize();
            let repo = app.world().resource::<WorldRepository>();
            write_save(repo, contents).await?;
            info!("Saved snapshot");
            return Ok(());
        }

        let start_time = Instant::now();
        app.update();

        if game_handle.is_finished() {
            game_handle.await??;
            return Err(anyhow!("failed to serve game connections"));
        }

        if lobby_handle.is_finished() {
            lobby_handle.await??;
            return Err(anyhow!("failed to serve lobby"));
        }

        if http_server_handle.is_finished() {
            http_server_handle.await??;
            return Err(anyhow!("failed to serve http API"));
        }

        let end_time = Instant::now();
        let frame_duration = end_time - start_time;
        if frame_duration < frame_wait {
           sleep(frame_wait - frame_duration).await;
        }
    }
}

async fn load_prefabs(
    app: &mut App,
    root_path: &Path,
    start_path: impl Into<PathBuf>,
) -> anyhow::Result<PrefabLibrary> {
    let mut to_visit = VecDeque::new();
    to_visit.push_back(start_path.into());

    let asset_server = app.world().resource::<AssetServer>().clone();
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

    let mut library = PrefabLibrary::default();
    for (name, handle) in queue {
        loop {
            match asset_server.get_load_state(&handle).unwrap() {
                LoadState::NotLoaded => unreachable!(),
                LoadState::Loading => {
                    app.update();
                    continue;
                },
                LoadState::Loaded => break,
                LoadState::Failed(err) => bail!("failed to load asset: {err}"),
            }
        }

        let fabricators = app.world().resource::<Assets<Fabricator>>();
        let fabricator = fabricators.get(&handle).unwrap().clone();
        library.insert(name, fabricator);
    }

    info!("Loaded {} prefabs", library.len());
    Ok(library)
}

#[derive(Component)]
struct StaticEntity(AssetPath<'static>);

async fn load_static_entities(
    app: &mut App,
    root_path: &Path,
    entities_path: impl Into<PathBuf>,
) -> anyhow::Result<()> {
    let mut to_visit = VecDeque::new();
    to_visit.push_back(entities_path.into());

    let asset_server = app.world().resource::<AssetServer>().clone();
    let mut count = 0;
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

    for (template, name, asset_path) in fab_queue {
        loop {
            match asset_server.get_load_state(&template).unwrap() {
                LoadState::NotLoaded => unreachable!(),
                LoadState::Loading => {
                    app.update();
                    continue;
                },
                LoadState::Loaded => break,
                LoadState::Failed(err) => bail!("failed to load asset: {err}"),
            }
        }

        let fabricators = app.world().resource::<Assets<Fabricator>>();
        let parameters = empty_reflect();
        let fabricate = Fabricate {
            fabricator: template,
            parameters: parameters.clone(),
        };
        let request = fabricate.to_request(&fabricators, Some(&asset_server))?
            .unwrap();
        app.world_mut()
            .spawn((
                Name::new(name),
                StaticEntity(asset_path),
                fabricate,
                WatchForFabricatorChanges,
                AssignNetId,
            ))
            .fabricate(request);
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
                warn!("failed to load updated fabricator: {err}");
                continue;
            }
        };

        info!("Reloaded {name}");
        commands.entity(entity).despawn_recursive();
        commands
            .spawn((
                name.clone(),
                StaticEntity(asset_path),
                fabricate,
                WatchForFabricatorChanges,
                AssignNetId,
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
