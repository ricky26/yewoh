use std::net::SocketAddr;

use anyhow::anyhow;
use async_trait::async_trait;
use log::{info, warn};
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::spawn;
use tokio::sync::mpsc;

use yewoh::protocol::{
    AccountLogin, AnyPacket, ClientVersion, new_io, Reader, Seed, SelectGameServer, ServerList,
    SwitchServer, Writer,
};

use crate::world::client::NewConnection;

pub fn accept_player_connections(listener: TcpListener) -> mpsc::UnboundedReceiver<NewConnection> {
    let (tx, rx) = mpsc::unbounded_channel();

    spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, address)) => {
                    info!("New connection from {address}");
                    let (reader, writer) = new_io(stream, true);
                    tx.send(NewConnection {
                        address,
                        reader,
                        writer,
                    }).ok();
                }
                Err(error) => {
                    warn!("error listening for client connections: {error}");
                    break;
                }
            }
        }
    });

    rx
}

#[async_trait]
pub trait Lobby {
    type User;

    async fn login(&mut self, username: String, password: String) -> anyhow::Result<Self::User>;
    async fn list_servers(&mut self, user: &Self::User) -> anyhow::Result<ServerList>;
    async fn allocate_session(
        &mut self, user: &Self::User, server_id: u16, seed: u32, client_version: ClientVersion,
    ) -> anyhow::Result<SwitchServer>;
}

pub async fn serve_lobby(mut lobby: impl Lobby, stream: TcpStream) -> anyhow::Result<()> {
    let (mut reader, mut writer) = new_io(stream, true);
    let seed = reader.recv(ClientVersion::default()).await?
        .into_downcast::<Seed>()
        .ok()
        .ok_or_else(|| anyhow!("expected seed as first packet"))?;

    let login = reader.recv(seed.client_version).await?
        .into_downcast::<AccountLogin>()
        .ok()
        .ok_or_else(|| anyhow!("expected account login attempt"))?;

    let user = lobby.login(login.username, login.password).await?;
    let servers = lobby.list_servers(&user).await?;
    writer.send(seed.client_version, &servers).await?;

    loop {
        let packet = reader.recv(seed.client_version).await?;
        if let Some(login_packet) = packet.downcast::<SelectGameServer>() {
            let session = lobby.allocate_session(
                &user,
                login_packet.server_id,
                seed.seed,
                seed.client_version
            ).await?;
            writer.send(seed.client_version, &session).await?;
        } else {
            return Err(anyhow!("unexpected lobby packet {:?}", packet))
        }
    }
}

pub async fn listen_for_lobby<L: Lobby + Send + 'static>(
    mut listener: TcpListener,
    mut lobby_factory: impl FnMut() -> L,
) -> anyhow::Result<()> where <L as Lobby>::User: Send + Sync {
    loop {
        let (stream, address) = listener.accept().await?;
        info!("New lobby connection from {address}");
        let lobby = lobby_factory();
        spawn(async move {
            if let Err(err) = serve_lobby(lobby, stream).await {
                warn!("error serving lobby: {:?}", err);
            }
            info!("Lobby connection disconnected {address}");
        });
    }
}

pub async fn serve_game(mut stream: TcpStream) -> anyhow::Result<()> {
    let token = stream.read_u32().await?;
    let (mut reader, mut writer) = new_io(stream, true);
    let seed = reader.recv(ClientVersion::default()).await?
        .into_downcast::<Seed>()
        .ok()
        .ok_or_else(|| anyhow!("expected seed as first packet"))?;

    let login = reader.recv(seed.client_version).await?
        .into_downcast::<AccountLogin>()
        .ok()
        .ok_or_else(|| anyhow!("expected account login attempt"))?;

    let user = lobby.login(login.username, login.password).await?;
    let servers = lobby.list_servers(&user).await?;
    writer.send(seed.client_version, &servers).await?;

    loop {
        let packet = reader.recv(seed.client_version).await?;
        if let Some(login_packet) = packet.downcast::<SelectGameServer>() {
            let session = lobby.allocate_session(
                &user,
                login_packet.server_id,
                seed.seed,
                seed.client_version
            ).await?;
            writer.send(seed.client_version, &session).await?;
        } else {
            return Err(anyhow!("unexpected lobby packet {:?}", packet))
        }
    }
}

pub async fn listen_for_game(mut listener: TcpListener) -> anyhow::Result<()> {
    loop {
        let (mut stream, address) = listener.accept().await?;
        info!("New game connection from {address}");
        spawn(async move {

            if let Err(err) = serve_lobby(lobby, stream).await {
                warn!("error serving game: {:?}", err);
            }
            info!("Game connection disconnected {address}");
        });
    }
}

pub struct RoutedConnection {
    pub address: SocketAddr,
    pub reader: Reader,
    pub writer: Writer,
    pub seed: u32,
    pub client_version: ClientVersion,
    pub first_packet: AnyPacket,
}

pub async fn to_routed_connection(
    new_connection: NewConnection,
) -> anyhow::Result<RoutedConnection> {
    let NewConnection { address, mut reader, mut writer } = new_connection;
    let mut client_version = ClientVersion::default();
    let mut seed = 0u32;

    loop {
        let packet = reader.recv(client_version).await?;

        /*
        if let Some(seed_packet) = packet.downcast::<Seed>() {
            seed = seed_packet.seed;
            client_version = seed_packet.client_version;
        } else if let Some(account_login) = packet.downcast::<AccountLogin>() {} else if let Some(_game_login) = packet.downcast::<GameServerLogin>() {} else if let Some(_create_character) = packet.downcast::<CreateCharacterEnhanced>() {
            client.send_packet(BeginEnterWorld {
                entity_id,
                body_type,
                position,
                direction,
                map_size: UVec2::new(5000, 5000),
            }.into());
            client.send_packet(ExtendedCommand::ChangeMap(0).into());
            client.send_packet(ChangeSeason { season: 0, play_sound: true }.into());

            client.send_packet(UpsertLocalPlayer {
                id: entity_id,
                body_type,
                server_id: 0,
                hue,
                flags: EntityFlags::empty(),
                position,
                direction,
            }.into());
            client.send_packet(UpsertEntityCharacter {
                id: entity_id,
                body_type,
                position,
                direction,
                hue,
                flags: EntityFlags::empty(),
                notoriety: Notoriety::Innocent,
                children: vec![],
            }.into());
            client.send_packet(UpsertEntityStats {
                id: entity_id,
                name: "CoolGuy".into(),
                max_info_level: 100,
                race_and_gender: 1,
                hp: 100,
                max_hp: 120,
                ..Default::default()
            }.into());

            client.send_packet(EndEnterWorld.into());

            client.send_packet(SetTime {
                hour: 12,
                minute: 16,
                second: 31,
            }.into());
        } else if let Some(version_request) = packet.downcast::<ClientVersionRequest>() {
            match ExtendedClientVersion::from_str(&version_request.version) {
                Ok(client_version) => client.client_version = client_version.client_version,
                Err(err) => {
                    log::warn!("Unable to parse client version '{}': {err}", &version_request.version);
                }
            }
        } else if let Some(request) = packet.downcast::<Move>() {
            client.send_packet(MoveConfirm {
                sequence: request.sequence,
                notoriety: Notoriety::Innocent,
            }.into());
        }
        */
    }
}

/*
pub trait ConnectionRouter {
    fn accept_lobby_connection(&self, connection: RoutedConnection);
    fn accept_game_connection(&self, connection: RoutedConnection);
}

pub fn route_connections(
    mut new_connections: mpsc::UnboundedReceiver<NewConnection>,
    router: impl ConnectionRouter,
) -> (mpsc::UnboundedReceiver<NewConnection>, mpsc::UnboundedReceiver<NewConnection>) {
    let (lobby_tx, lobby_rx) = mpsc::unbounded_channel();
    let (game_tx, game_rx) = mpsc::unbounded_channel();

    spawn(async move {
        while let Some(new_connection) = new_connections.recv().await {
            spawn(async move {
                let mut reader = new_connection.reader;

                while let Some(packet) = reader.recv().await {
                    if let Some(seed) = packet.downcast::<Seed>() {
                        client.seed = seed.seed;
                        client.client_version = seed.client_version;
                    } else if let Some(account_login) = packet.downcast::<AccountLogin>() {
                        log::info!("New login attempt for {}", &account_login.username);
                        client.send_packet(ServerList {
                            system_info_flags: 0x5d,
                            game_servers: vec![
                                GameServer {
                                    server_name: "My Server".into(),
                                    ..Default::default()
                                },
                            ],
                            ..Default::default()
                        }.into());
                    } else if let Some(_game_server) = packet.downcast::<SelectGameServer>() {
                        // TODO: pass this across using token
                        client.send_packet(SwitchServer {
                            ip: Ipv4Addr::LOCALHOST.into(),
                            port: 2593,
                            token: 7,
                        }.into());
                    } else if let Some(_game_login) = packet.downcast::<GameServerLogin>() {
                        client.enable_compression();

                        // HACK: assume new client for now
                        client.client_version = ClientVersion::new(8, 0, 0, 0);
                        client.send_packet(ClientVersionRequest::default().into());

                        client.send_packet(SupportedFeatures {
                            feature_flags: FeatureFlags::T2A
                                | FeatureFlags::UOR
                                | FeatureFlags::LBR
                                | FeatureFlags::AOS
                                | FeatureFlags::SE
                                | FeatureFlags::ML
                                | FeatureFlags::NINTH_AGE
                                | FeatureFlags::LIVE_ACCOUNT
                                | FeatureFlags::SA
                                | FeatureFlags::HS
                                | FeatureFlags::GOTHIC
                                | FeatureFlags::RUSTIC
                                | FeatureFlags::JUNGLE
                                | FeatureFlags::SHADOWGUARD
                                | FeatureFlags::TOL
                                | FeatureFlags::EJ,
                        }.into());
                        client.send_packet(CharacterList {
                            characters: vec![None; 5],
                            cities: vec![StartingCity {
                                index: 0,
                                city: "My City".into(),
                                building: "My Building".into(),
                                position: UVec3::new(0, 1, 2),
                                map_id: 0,
                                description_id: 0,
                            }],
                        }.into());
                    } else if let Some(_create_character) = packet.downcast::<CreateCharacterEnhanced>() {
                        client.send_packet(BeginEnterWorld {
                            entity_id,
                            body_type,
                            position,
                            direction,
                            map_size: UVec2::new(5000, 5000),
                        }.into());
                        client.send_packet(ExtendedCommand::ChangeMap(0).into());
                        client.send_packet(ChangeSeason { season: 0, play_sound: true }.into());

                        client.send_packet(UpsertLocalPlayer {
                            id: entity_id,
                            body_type,
                            server_id: 0,
                            hue,
                            flags: EntityFlags::empty(),
                            position,
                            direction,
                        }.into());
                        client.send_packet(UpsertEntityCharacter {
                            id: entity_id,
                            body_type,
                            position,
                            direction,
                            hue,
                            flags: EntityFlags::empty(),
                            notoriety: Notoriety::Innocent,
                            children: vec![],
                        }.into());
                        client.send_packet(UpsertEntityStats {
                            id: entity_id,
                            name: "CoolGuy".into(),
                            max_info_level: 100,
                            race_and_gender: 1,
                            hp: 100,
                            max_hp: 120,
                            ..Default::default()
                        }.into());

                        client.send_packet(EndEnterWorld.into());

                        client.send_packet(SetTime {
                            hour: 12,
                            minute: 16,
                            second: 31,
                        }.into());
                    } else if let Some(version_request) = packet.downcast::<ClientVersionRequest>() {
                        match ExtendedClientVersion::from_str(&version_request.version) {
                            Ok(client_version) => client.client_version = client_version.client_version,
                            Err(err) => {
                                log::warn!("Unable to parse client version '{}': {err}", &version_request.version);
                            }
                        }
                    } else if let Some(request) = packet.downcast::<Move>() {
                        client.send_packet(MoveConfirm {
                            sequence: request.sequence,
                            notoriety: Notoriety::Innocent,
                        }.into());
                    }
                }
            });
        }
    });

    (lobby_rx, game_rx)
}
 */
