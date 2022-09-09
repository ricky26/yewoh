use std::collections::{hash_map, HashMap};
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::anyhow;
use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};

use yewoh::protocol::{ClientVersion, GameServer, ServerList, SwitchServer};
use yewoh_server::Lobby;

pub struct NewSessionRequest {
    pub username: String,
    pub seed: u32,
    pub client_version: ClientVersion,
    pub token: u32,
    pub done: oneshot::Sender<anyhow::Result<()>>,
}

#[derive(Debug)]
struct LobbyShared {
    server_name: String,
    next_token: AtomicU32,
    external_ip: Ipv4Addr,
    game_port: u16,
    timezone: u8,
    new_session_tx: mpsc::UnboundedSender<NewSessionRequest>,
}

#[derive(Debug, Clone)]
pub struct LocalLobby {
    shared: Arc<LobbyShared>,
}

impl LocalLobby {
    pub fn new(
        server_name: String,
        external_ip: Ipv4Addr,
        game_port: u16,
        timezone: u8,
        new_session_tx: mpsc::UnboundedSender<NewSessionRequest>,
    ) -> LocalLobby {
        let shared = Arc::new(LobbyShared {
            server_name,
            next_token: AtomicU32::new(1),
            external_ip,
            game_port,
            timezone,
            new_session_tx,
        });

        LocalLobby { shared }
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
                    load_percent: 0,
                    timezone: 0,
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

struct PendingSession {
    pub username: String,
    pub seed: u32,
    pub client_version: ClientVersion,
}

pub struct SessionAllocator {
    pending_sessions: HashMap<u32, PendingSession>,
}

impl SessionAllocator {
    pub fn allocate_session(&mut self, session: NewSessionRequest) -> anyhow::Result<()> {
        let entry = self.pending_sessions.entry(session.token);
        if matches!(entry, hash_map::Entry::Vacant(_)) {
            entry.or_insert(PendingSession {
                username: session.username,
                seed: session.seed,
                client_version: Default::default(),
            });
            Ok(())
        } else {
            Err(anyhow!("duplicate login token"))
        }
    }
}