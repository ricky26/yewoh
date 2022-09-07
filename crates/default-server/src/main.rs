use std::fs::File;
use std::path::PathBuf;

use bevy_app::App;
use clap::Parser;
use log::info;
use memmap2::Mmap;
use tokio::net::TcpListener;

use yewoh::assets::uop::UopBuffer;
use yewoh_server::accept_player_connections;
use yewoh_server::world::client::PlayerServer;
use yewoh_server::world::ServerPlugin;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    #[clap(short, long, default_value = "data", env = "UO_DATA")]
    /// Path to the Ultima Online Classic data.
    uo_data_path: PathBuf,

    #[clap(short, long, default_value = "0.0.0.0:2593", env = "YEWOH_CLIENT_BIND")]
    client_bind: String,
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

    let client_listener = rt.block_on(TcpListener::bind(args.client_bind))?;
    let new_connections = accept_player_connections(client_listener);

    let mut app = App::new();
    app
        .add_plugin(ServerPlugin)
        .insert_resource(PlayerServer::new(new_connections));

    loop {
        app.update();
    }
}
