//! Local WebSocket RPC prototype for mkagent protocol 1.0.
//! Reconnection and the complete typed facade are tracked by T-1-02/T-1-03.
#![deny(missing_docs)]
mod transport;
use agentkit_protocol::{Envelope, EnvelopeKind, WireError};
use futures::channel::oneshot;
use secrecy::SecretString;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub use agentkit_protocol::Endpoint;
/// Client connection configuration.
#[derive(Clone, Debug)]
pub struct ClientOptions {
    /// Local backend connection.
    pub endpoint: Endpoint,
    /// Workspace bound to this window.
    pub workspace_id: String,
    /// Ordinary invocation timeout (default 30 seconds).
    pub request_timeout: Duration,
    /// Total connect/handshake timeout (default 10 seconds).
    pub connect_timeout: Duration,
}
impl ClientOptions {
    /// Build a loopback-only configuration. No unimplemented capabilities are advertised.
    pub fn local(url: String, token: SecretString, workspace_id: String) -> Self {
        Self {
            endpoint: Endpoint { url, token },
            workspace_id,
            request_timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
        }
    }
}
/// Observable transport status; this prototype does not reconnect automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Opening the socket.
    Connecting,
    /// Waiting for authentication acknowledgement.
    Handshaking,
    /// Authenticated and available for business calls.
    Connected,
    /// Closed or failed; pending calls are rejected.
    Disconnected,
}
/// Invocation or transport failure.
#[derive(Debug, Clone, thiserror::Error)]
pub enum RpcError {
    /// Structured server error.
    #[error("{0}")]
    Wire(#[from] WireError),
    /// Request or handshake exceeded its deadline.
    #[error("RPC request timed out")]
    Timeout,
    /// Connection ended before a response arrived.
    #[error("RPC client disconnected")]
    Disconnected,
    /// Server does not register this channel.
    #[error("RPC channel unavailable: {0}")]
    ChannelUnavailable(String),
    /// Serialized UTF-8 frame exceeds the server-advertised size.
    #[error("RPC frame exceeds server size limit")]
    TooLarge,
    /// Invalid wire payload.
    #[error("Invalid RPC payload")]
    Decode(Arc<serde_json::Error>),
    /// Failed socket establishment or I/O.
    #[error("RPC transport failed: {0}")]
    Transport(String),
}
impl From<serde_json::Error> for RpcError {
    fn from(error: serde_json::Error) -> Self {
        Self::Decode(Arc::new(error))
    }
}
type Subscribers = Arc<Mutex<HashMap<String, Vec<async_channel::Sender<Vec<Value>>>>>>;
pub(crate) fn locked<T>(value: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    value
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
pub(crate) enum Command {
    Invoke {
        frame: Box<Envelope>,
        deadline: Instant,
        result: oneshot::Sender<Result<Value, RpcError>>,
    },
}
/// Cheap cloneable handle to an independent smol IO thread.
#[derive(Clone)]
pub struct RpcClient {
    commands: async_channel::Sender<Command>,
    subscribers: Subscribers,
    state: Arc<Mutex<ConnectionState>>,
    timeout: Duration,
    stop: async_channel::Sender<()>,
    failure: Arc<Mutex<Option<RpcError>>>,
}
impl RpcClient {
    /// Start connecting without blocking the calling (GUI) thread.
    pub fn connect(options: ClientOptions) -> Self {
        let (commands, receiver) = async_channel::unbounded();
        let (stop, stopping) = async_channel::bounded(1);
        let failure = Arc::new(Mutex::new(None));
        let thread_failure = failure.clone();
        let subscribers = Arc::new(Mutex::new(HashMap::new()));
        let state = Arc::new(Mutex::new(ConnectionState::Connecting));
        let handle = Self {
            commands,
            subscribers: subscribers.clone(),
            state: state.clone(),
            timeout: options.request_timeout,
            stop,
            failure: failure.clone(),
        };
        let thread_state = state.clone();
        let result = std::thread::Builder::new()
            .name("agentkit-rpc".into())
            .spawn(move || {
                smol::block_on(transport::run(
                    options,
                    receiver,
                    subscribers,
                    thread_state,
                    stopping,
                    thread_failure,
                ));
            });
        if result.is_err() {
            *locked(&failure) = Some(RpcError::Transport("Unable to start RPC thread".into()));
            *locked(&state) = ConnectionState::Disconnected;
        }
        handle
    }
    /// Invoke a channel with the configured ordinary timeout.
    pub async fn invoke<T: DeserializeOwned>(
        &self,
        channel: &str,
        args: Vec<Value>,
    ) -> Result<T, RpcError> {
        self.invoke_timeout(channel, args, self.timeout).await
    }
    /// Invoke with a channel-specific timeout; writes are never automatically retried.
    pub async fn invoke_timeout<T: DeserializeOwned>(
        &self,
        channel: &str,
        args: Vec<Value>,
        timeout: Duration,
    ) -> Result<T, RpcError> {
        if let Some(error) = self.last_error() {
            return Err(error);
        }
        let frame = Envelope {
            id: uuid::Uuid::new_v4().to_string(),
            kind: EnvelopeKind::Request,
            channel: Some(channel.into()),
            args: Some(args),
            ..Envelope::default()
        };
        let (send, receive) = oneshot::channel();
        self.commands
            .send(Command::Invoke {
                frame: Box::new(frame),
                deadline: Instant::now() + timeout,
                result: send,
            })
            .await
            .map_err(|_| self.last_error().unwrap_or(RpcError::Disconnected))?;
        let value = smol::future::race(
            async {
                receive
                    .await
                    .map_err(|_| self.last_error().unwrap_or(RpcError::Disconnected))?
            },
            async {
                smol::Timer::after(timeout).await;
                Err(RpcError::Timeout)
            },
        )
        .await?;
        Ok(serde_json::from_value(value)?)
    }
    /// Subscribe to a push channel. Each subscriber receives its own unbounded queue.
    pub fn subscribe(&self, channel: &str) -> async_channel::Receiver<Vec<Value>> {
        let (send, receive) = async_channel::unbounded();
        let mut table = locked(&self.subscribers);
        if self.state() != ConnectionState::Disconnected {
            table.entry(channel.into()).or_default().push(send);
        }

        receive
    }
    /// Current connection state.
    pub fn state(&self) -> ConnectionState {
        *locked(&self.state)
    }
    /// Cause of the terminal failure, preserving server authentication/protocol codes.
    pub fn last_error(&self) -> Option<RpcError> {
        locked(&self.failure).clone()
    }
    /// Request closure; all pending invocations finish with Disconnected.
    pub fn close(&self) {
        let _ = self.stop.try_send(());
    }
}

/// Typed subset used by the phase-zero native workbench.
pub mod api;
