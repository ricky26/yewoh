use std::collections::{hash_map, HashMap};
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};

use anyhow::anyhow;
use async_trait::async_trait;
use log::{info, warn};
use tokio::net::{TcpListener, TcpStream};
use tokio::spawn;
use tokio::sync::{mpsc, oneshot};

use yewoh::protocol::{
    new_io, AccountLogin, ClientVersion, GameServer, Reader, Seed, SelectGameServer, ServerList,
    SwitchServer, Writer,
};
use crate::game_server::NewSessionAttempt;

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
    listener: TcpListener,
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

pub struct NewSessionRequest {
    pub username: String,
    pub seed: u32,
    pub client_version: ClientVersion,
    pub token: u32,
    pub done: oneshot::Sender<anyhow::Result<()>>,
}

#[derive(Debug)]
struct LocalLobbyShared {
    server_name: String,
    next_token: AtomicU32,
    external_ip: Ipv4Addr,
    game_port: u16,
    timezone: u8,
    load: AtomicU8,
    new_session_tx: mpsc::UnboundedSender<NewSessionRequest>,
}

#[derive(Debug, Clone)]
pub struct LocalLobby {
    shared: Arc<LocalLobbyShared>,
}

impl LocalLobby {
    pub fn new(
        server_name: String,
        external_ip: Ipv4Addr,
        game_port: u16,
        timezone: u8,
        new_session_tx: mpsc::UnboundedSender<NewSessionRequest>,
    ) -> LocalLobby {
        let shared = Arc::new(LocalLobbyShared {
            server_name,
            next_token: AtomicU32::new(1),
            external_ip,
            game_port,
            timezone,
            load: AtomicU8::new(0),
            new_session_tx,
        });

        LocalLobby { shared }
    }

    pub fn set_load(&self, load: u8) {
        self.shared.load.store(load, Ordering::Relaxed);
    }
}

#[async_trait]
impl Lobby for LocalLobby {
    type User = String;

    async fn login(&mut self, username: String, _password: String) -> anyhow::Result<Self::User> {
        Ok(username)
    }

    async fn list_servers(&mut self, _user: &Self::User) -> anyhow::Result<ServerList> {
        Ok(ServerList {
            system_info_flags: 0,
            game_servers: vec![
                GameServer {
                    server_index: 0,
                    server_name: self.shared.server_name.to_string(),
                    load_percent: self.shared.load.load(Ordering::Relaxed),
                    timezone: self.shared.timezone,
                    ip: self.shared.external_ip.into(),
                }
            ],
        })
    }

    async fn allocate_session(&mut self,
        user: &Self::User,
        _server_id: u16,
        seed: u32,
        client_version: ClientVersion,
    ) -> anyhow::Result<SwitchServer> {
        let (tx, rx) = oneshot::channel();
        let token = self.shared.next_token.fetch_add(1, Ordering::Relaxed);
        let request = NewSessionRequest {
            username: user.to_string(),
            client_version,
            seed,
            token,
            done: tx,
        };
        self.shared.new_session_tx.send(request)
            .map_err(|_| anyhow!("game server is gone"))?;
        rx.await??;

        Ok(SwitchServer {
            ip: self.shared.external_ip.into(),
            port: self.shared.game_port,
            token,
        })
    }
}

pub struct NewSession {
    pub address: SocketAddr,
    pub reader: Reader,
    pub writer: Writer,
    pub username: String,
    pub password: String,
    pub seed: u32,
    pub client_version: ClientVersion,
}

#[derive(Debug)]
struct PendingSession {
    pub username: String,
    pub seed: u32,
    pub client_version: ClientVersion,
}

#[derive(Debug)]
pub struct SessionAllocator {
    pending_sessions: HashMap<u32, PendingSession>,
}

impl SessionAllocator {
    pub fn new() -> SessionAllocator {
        Self {
            pending_sessions: HashMap::new(),
        }
    }

    pub fn allocate_session(&mut self, session: NewSessionRequest) {
        let entry = self.pending_sessions.entry(session.token);
        let result = if matches!(entry, hash_map::Entry::Vacant(_)) {
            entry.or_insert(PendingSession {
                username: session.username,
                seed: session.seed,
                client_version: session.client_version,
            });
            Ok(())
        } else {
            Err(anyhow!("duplicate login token"))
        };

        session.done.send(result).ok();
    }

    pub fn start_session(&mut self, session: NewSessionAttempt) -> anyhow::Result<NewSession> {
        let test_session = match self.pending_sessions.get(&session.token) {
            Some(x) => x,
            None => return Err(anyhow!("no such session token")),
        };

        if test_session.username != session.username {
            return Err(anyhow!("wrong user for session"));
        }

        let request = self.pending_sessions.remove(&session.token).unwrap();
        Ok(NewSession {
            address: session.address,
            reader: session.reader,
            writer: session.writer,
            username: session.username,
            password: session.password,
            seed: request.seed,
            client_version: request.client_version,
        })
    }
}
