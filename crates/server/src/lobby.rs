use std::collections::{hash_map, HashMap};
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};

use anyhow::anyhow;
use async_trait::async_trait;
use log::{info, warn};
use tokio::net::{TcpListener, TcpStream};
use tokio::spawn;
use tokio::sync::{mpsc, oneshot};

use yewoh::protocol::{AccountLogin, ClientVersion, GameServer, GameServerLogin, LoginError, new_io, Seed, SelectGameServer, ServerList, SwitchServer};
use yewoh::protocol::encryption::Encryption;
use yewoh::types::FixedString;

#[async_trait]
pub trait AccountRepository {
    async fn login(&mut self, username: &str, password: &str) -> anyhow::Result<()>;
}

#[async_trait]
pub trait ServerRepository {
    async fn list_servers(&mut self, username: &str) -> anyhow::Result<ServerList>;
    async fn allocate_session(
        &mut self, username: &str, server_id: u16, seed: u32, client_version: ClientVersion,
    ) -> anyhow::Result<SwitchServer>;
}

pub async fn serve_lobby(
    mut servers: impl ServerRepository, mut accounts: impl AccountRepository,
    encrypted: bool, stream: TcpStream,
) -> anyhow::Result<()> {
    let (mut reader, mut writer) = new_io(stream, true);
    let seed = reader.recv(ClientVersion::default()).await?
        .into_downcast::<Seed>()
        .ok()
        .ok_or_else(|| anyhow!("expected seed as first packet"))?;

    if encrypted {
        let encryption = Encryption::new(seed.client_version, seed.seed, true);
        reader.set_encryption(Some(encryption));
    }

    let login = reader.recv(seed.client_version).await?
        .into_downcast::<AccountLogin>()
        .ok()
        .ok_or_else(|| anyhow!("expected account login attempt"))?;

    if let Err(err) = accounts.login(&login.username, &login.password).await {
        writer.send(seed.client_version, &LoginError::InvalidUsernamePassword).await.ok();
        return Err(err);
    }

    let server_list = servers.list_servers(&login.username).await?;
    writer.send(seed.client_version, &server_list).await?;

    loop {
        let packet = reader.recv(seed.client_version).await?;
        if let Some(login_packet) = packet.downcast::<SelectGameServer>() {
            let session = servers.allocate_session(
                &login.username,
                login_packet.server_id,
                seed.seed,
                seed.client_version,
            ).await?;
            writer.send(seed.client_version, &session).await?;
        } else {
            writer.send(seed.client_version, &LoginError::CommunicationProblem).await.ok();
            return Err(anyhow!("unexpected lobby packet {:?}", packet));
        }
    }
}

pub async fn listen_for_lobby<
    S: ServerRepository + Send + 'static,
    A: AccountRepository + Send + 'static,
>(
    listener: TcpListener,
    encrypted: bool,
    mut servers_factory: impl FnMut() -> S,
    mut accounts_factory: impl FnMut() -> A,
) -> anyhow::Result<()> {
    loop {
        let (stream, address) = listener.accept().await?;
        info!("New lobby connection from {address}");
        let servers = servers_factory();
        let accounts = accounts_factory();
        spawn(async move {
            if let Err(err) = serve_lobby(servers, accounts, encrypted, stream).await {
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
struct LocalServerRepositoryInner {
    server_name: String,
    next_token: AtomicU32,
    external_ip: Ipv4Addr,
    game_port: u16,
    timezone: u8,
    load: AtomicU8,
    new_session_tx: mpsc::UnboundedSender<NewSessionRequest>,
}

#[derive(Debug, Clone)]
pub struct LocalServerRepository {
    shared: Arc<LocalServerRepositoryInner>,
}

impl LocalServerRepository {
    pub fn new(
        server_name: String,
        external_ip: Ipv4Addr,
        game_port: u16,
        timezone: u8,
        new_session_tx: mpsc::UnboundedSender<NewSessionRequest>,
    ) -> LocalServerRepository {
        let shared = Arc::new(LocalServerRepositoryInner {
            server_name,
            next_token: AtomicU32::new(1),
            external_ip,
            game_port,
            timezone,
            load: AtomicU8::new(0),
            new_session_tx,
        });

        LocalServerRepository { shared }
    }

    pub fn set_load(&self, load: u8) {
        self.shared.load.store(load, Ordering::Relaxed);
    }
}

#[async_trait]
impl ServerRepository for LocalServerRepository {
    async fn list_servers(&mut self, _user: &str) -> anyhow::Result<ServerList> {
        Ok(ServerList {
            system_info_flags: 0,
            game_servers: vec![
                GameServer {
                    server_index: 0,
                    server_name: FixedString::from_str(&self.shared.server_name),
                    load_percent: self.shared.load.load(Ordering::Relaxed),
                    timezone: self.shared.timezone,
                    ip: self.shared.external_ip.into(),
                }
            ],
        })
    }

    async fn allocate_session(
        &mut self, username: &str, _server_id: u16, seed: u32, client_version: ClientVersion,
    ) -> anyhow::Result<SwitchServer> {
        let (tx, rx) = oneshot::channel();
        let token = self.shared.next_token.fetch_add(1, Ordering::Relaxed);
        let request = NewSessionRequest {
            username: username.to_string(),
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
    pub username: String,
}

#[derive(Debug)]
struct PendingSession {
    pub username: String,
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
                client_version: session.client_version,
            });
            Ok(())
        } else {
            Err(anyhow!("duplicate login token"))
        };

        session.done.send(result).ok();
    }

    pub fn client_version_for_token(&self, token: u32) -> Option<ClientVersion> {
        self.pending_sessions.get(&token).map(|t| t.client_version)
    }

    pub fn start_session(&mut self, token: u32, login: GameServerLogin) -> anyhow::Result<NewSession> {
        let test_session = match self.pending_sessions.get(&token) {
            Some(x) => x,
            None => return Err(anyhow!("no such session token")),
        };

        if login.username.as_str() != &test_session.username {
            return Err(anyhow!("wrong user for session"));
        }

        self.pending_sessions.remove(&token);
        Ok(NewSession {
            username: login.username.to_string(),
        })
    }
}
