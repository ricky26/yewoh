use std::fs::File;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::anyhow;
use bevy_app::App;
use clap::Parser;
use futures::future::join;
use log::info;
use memmap2::Mmap;
use tokio::net::{lookup_host, TcpListener};
use tokio::sync::mpsc;

use yewoh::assets::uop::UopBuffer;
use yewoh_default_game::DefaultGamePlugin;
use yewoh_server::{accept_player_connections, listen_for_lobby};
use yewoh_server::world::client::PlayerServer;
use yewoh_server::world::ServerPlugin;

mod lobby;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Path to the Ultima Online Classic data.
    #[clap(short, long, default_value = "data", env = "UO_DATA")]
    uo_data_path: PathBuf,

    /// The display name of this game server.
    #[clap(short, long, default_value = "Yewoh Server", env = "YEWOH_SERVER_NAME")]
    server_display_name: String,

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
    let art_uop_path = args.uo_data_path.join("artLegacyMUL.uop");
    let art_uop_file = File::open(art_uop_path)?;
    let mmap = unsafe { Mmap::map(&art_uop_file)? };
    let _uop = UopBuffer::try_from_backing(mmap)?;

    let external_ip = rt.block_on(lookup_host(format!("{}:0", &args.advertise_address)))?
        .filter_map(|entry| match entry {
            SocketAddr::V4(v4) => Some(*v4.ip()),
            _ => None,
        })
        .next()
        .ok_or_else(|| anyhow!("couldn't resolve {}", &args.advertise_address))?;

    let game_port = SocketAddr::from_str(&args.game_bind)?.port();
    let (tx, _) = mpsc::unbounded_channel();
    let lobby = lobby::LocalLobby::new(
        args.server_display_name, external_ip, game_port, 0, tx);

    let (lobby_listener, game_listener) = rt.block_on(join(
        TcpListener::bind(&args.lobby_bind),
        TcpListener::bind(&args.game_bind),
    ));

    let lobby_listener = lobby_listener?;
    let lobby_handle = rt.spawn(listen_for_lobby(lobby_listener, move || lobby.clone()));
    let new_connections = accept_player_connections(game_listener?);

    let mut app = App::new();
    app
        .add_plugin(ServerPlugin)
        .add_plugin(DefaultGamePlugin)
        .insert_resource(PlayerServer::new(new_connections));

    info!("Listening for http connections on {}", &args.http_bind);
    info!("Listening for lobby connections on {}", &args.lobby_bind);
    info!("Listening for game connections on {}", &args.game_bind);

    loop {
        app.update();

        if lobby_handle.is_finished() {
            rt.block_on(lobby_handle)?;
            return Err(anyhow!("failed to serve lobby"));
        }
    }
}
