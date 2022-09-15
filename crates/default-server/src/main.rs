use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::anyhow;
use bevy_app::App;
use bevy_ecs::prelude::*;
use clap::Parser;
use futures::future::join;
use log::info;
use tokio::net::{lookup_host, TcpListener};
use tokio::sync::mpsc;

use yewoh::assets::tiles::load_tile_data;
use yewoh_default_game::data::static_data;
use yewoh_default_game::DefaultGamePlugin;
use yewoh_server::game_server::listen_for_game;
use yewoh_server::lobby::{listen_for_lobby, LocalLobby};
use yewoh_server::world::map::{Chunk, create_map_entities, create_statics, Static};
use yewoh_server::world::net::NetServer;
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
    #[clap(short, long, default_value = "0.0.0.0:2595", env = "YEWOH_HTTP_BIND")]
    http_bind: String,

    /// The bind address for the lobby server.
    #[clap(short, long, default_value = "0.0.0.0:2593", env = "YEWOH_LOBBY_BIND")]
    lobby_bind: String,

    /// The bind address for the game server.
    #[clap(short, long, default_value = "0.0.0.0:2594", env = "YEWOH_GAME_BIND")]
    game_bind: String,
}

fn main() -> anyhow::Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let _guard = rt.enter();

    env_logger::builder()
        .parse_filters("warn,yewoh=info,yewoh-server=info,yewoh-default-game=info,yewoh-default-server=info")
        .parse_default_env()
        .init();

    let args = Args::parse();
    let mut app = App::new();
    app
        .add_plugin(ServerPlugin)
        .add_plugin(DefaultGamePlugin);

    let static_data = rt.block_on(static_data::load_from_directory(&args.data_path))?;
    let map_infos = static_data.maps.map_infos();
    let tile_data = rt.block_on(load_tile_data(&args.uo_data_path))?;

    info!("Loading map data...");
    rt.block_on(create_map_entities(&mut app.world, &map_infos, &args.uo_data_path))?;
    info!("Loading statics...");
    rt.block_on(create_statics(&mut app.world, &map_infos, &tile_data, &args.uo_data_path))?;

    let mut query = app.world.query_filtered::<(), With<Chunk>>();
    info!("Spawned {} map chunks", query.iter(&app.world).count());
    let mut query = app.world.query_filtered::<(), With<Static>>();
    info!("Spawned {} statics", query.iter(&app.world).count());

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
        .insert_resource(NetServer::new(args.encryption, new_session_requests, new_session_rx))
        .insert_resource(map_infos)
        .insert_resource(static_data)
        .insert_resource(tile_data);

    info!("Listening for http connections on {}", &args.http_bind);
    info!("Listening for lobby connections on {}", &args.lobby_bind);
    info!("Listening for game connections on {}", &args.game_bind);

    loop {
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
    }
}
