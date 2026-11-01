//! Observable transport metadata and lifecycle broadcasts.
use super::*;
/// Transport state. Spawning/Restarting are reserved for the sidecar owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Sidecar startup, managed by the application.
    Spawning,
    /// Opening a socket.
    Connecting,
    /// Waiting for authentication acknowledgement.
    Handshaking,
    /// Authenticated.
    Connected,
    /// Retry countdown. Attempt is one-based; delay is the scheduled duration.
    Reconnecting {
        /// Consecutive retry count.
        attempt: u32,
        /// Scheduled delay in milliseconds.
        next_retry_ms: u64,
    },
    /// Sidecar replacement, managed by the application.
    Restarting {
        /// Restart attempt.
        attempt: u32,
    },
    /// Authentication/configuration/protocol failure requiring intervention.
    Fatal {
        /// Redacted category, details available through last_error.
        reason: String,
    },
    /// Explicitly closed; no further retries.
    Disconnected,
}
/// Metadata replaced atomically on each handshake.
#[derive(Debug, Clone)]
pub struct ConnectionSnapshot {
    /// Current lifecycle state.
    pub state: ConnectionState,
    /// Server-assigned identity.
    pub client_id: Option<String>,
    /// Backend version.
    pub server_version: Option<String>,
    /// Advertised channel list, absent for legacy servers.
    pub registered_channels: Option<Vec<String>>,
    /// Current outbound message limit (legacy fallback 16MiB).
    pub max_message_bytes: u64,
    /// Maximum accepted event sequence for this server generation.
    pub last_seen_seq: u64,
    /// Increments for each successful handshake.
    pub recovery_id: u64,
    /// Last handshake generation requiring reload; retained across later resumes.
    pub refresh_epoch: u64,
    /// Snapshot/cache reload required after the latest handshake.
    pub needs_refresh: bool,
}
pub(crate) struct Info {
    pub snapshot: ConnectionSnapshot,
    pub failure: Option<RpcError>,
    pub generation: u64,
}
pub(crate) struct Shared {
    pub info: Mutex<Info>,
    pub subscribers: Subscribers,
    pub capabilities: Mutex<crate::capability::Handlers>,
    states: Mutex<Vec<async_channel::Sender<ConnectionState>>>,
}
impl Shared {
    pub fn new(capabilities: crate::capability::Handlers) -> Self {
        Self {
            info: Mutex::new(Info {
                snapshot: ConnectionSnapshot {
                    state: ConnectionState::Connecting,
                    client_id: None,
                    server_version: None,
                    registered_channels: None,
                    max_message_bytes:
                        agentkit_protocol::constants::limits::DEFAULT_MAX_MESSAGE_BYTES,
                    last_seen_seq: 0,
                    recovery_id: 0,
                    refresh_epoch: 0,
                    needs_refresh: true,
                },
                failure: None,
                generation: 0,
            }),
            subscribers: Arc::new(Mutex::new(HashMap::new())),
            states: Mutex::new(Vec::new()),
            capabilities: Mutex::new(capabilities),
        }
    }
    pub fn invalidate(&self, commands: &async_channel::Receiver<Command>, reason: RpcError) {
        let mut info = locked(&self.info);
        // Binding and enqueue use this same lock. Never leave a lost generation
        // observable as Connected while its socket is closing.
        if !matches!(
            info.snapshot.state,
            ConnectionState::Fatal { .. }
                | ConnectionState::Disconnected
                | ConnectionState::Connecting
        ) {
            info.snapshot.state = ConnectionState::Connecting;
            locked(&self.states).retain(|s| s.try_send(ConnectionState::Connecting).is_ok());
        }
        info.generation = info.generation.wrapping_add(1);
        info.failure = Some(reason.clone());
        info.snapshot.registered_channels = None;
        info.snapshot.server_version = None;
        while let Ok(Command::Invoke { result, .. }) = commands.try_recv() {
            let _ = result.send(Err(reason.clone()));
        }
    }
    pub fn set_state(&self, state: ConnectionState) {
        let mut info = locked(&self.info);
        if info.snapshot.state != state {
            info.snapshot.state = state.clone();
            locked(&self.states).retain(|s| s.try_send(state.clone()).is_ok());
        }
    }
    pub fn state_changes(&self) -> async_channel::Receiver<ConnectionState> {
        let (send, receive) = async_channel::unbounded();
        let info = locked(&self.info);
        let _ = send.try_send(info.snapshot.state.clone());
        if info.snapshot.state != ConnectionState::Disconnected {
            locked(&self.states).push(send);
        }
        receive
    }
    pub fn publish(&self, channel: &str, args: Vec<Value>) {
        if let Some(list) = locked(&self.subscribers).get_mut(channel) {
            list.retain(|s| s.try_send(args.clone()).is_ok());
        }
    }
    pub fn finish(&self) {
        self.set_state(ConnectionState::Disconnected);
        locked(&self.subscribers).clear();
        locked(&self.states).clear();
    }
}
