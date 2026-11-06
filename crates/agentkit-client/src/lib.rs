//! Local WebSocket RPC client with reconnect, replay metadata and bounded IO.
#![deny(missing_docs)]
mod capability;
/// Opt-in, background-only event recording for local development.
pub mod recording;
mod state;
mod transport;
use agentkit_protocol::{Envelope, EnvelopeKind, WireError};
pub use capability::CapabilityHandler;
use futures::channel::oneshot;
use secrecy::SecretString;
use serde::de::DeserializeOwned;
use serde_json::Value;
use state::Shared;
pub use state::{ConnectionSnapshot, ConnectionState};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub use agentkit_protocol::Endpoint;
/// Client connection configuration.
#[derive(Clone)]
pub struct ClientOptions {
    /// Local backend connection.
    pub endpoint: Endpoint,
    /// Workspace bound to this window.
    pub workspace_id: String,
    /// Ordinary invocation timeout (default 30 seconds).
    pub request_timeout: Duration,
    /// Total connect/handshake timeout (default 10 seconds).
    pub connect_timeout: Duration,
    /// Maximum reconnect backoff (default 30 seconds).
    pub max_reconnect_delay: Duration,
    /// Implemented reverse RPCs, registered before the initial handshake. Browser is filtered.
    pub capabilities: std::collections::BTreeMap<String, CapabilityHandler>,
    /// Maximum time for one reverse RPC (default 30 seconds).
    pub capability_timeout: Duration,
}
impl std::fmt::Debug for ClientOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientOptions")
            .field("endpoint", &self.endpoint)
            .field("workspace_id", &self.workspace_id)
            .field(
                "capabilities",
                &self.capabilities.keys().collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}
impl ClientOptions {
    /// Build a loopback-only configuration. No unimplemented capabilities are advertised.
    pub fn local(url: String, token: SecretString, workspace_id: String) -> Self {
        Self {
            endpoint: Endpoint { url, token },
            workspace_id,
            request_timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            max_reconnect_delay: Duration::from_secs(30),
            capabilities: Default::default(),
            capability_timeout: Duration::from_secs(30),
        }
    }
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
    /// Invalid endpoint configuration; requires replacement.
    #[error("Invalid local RPC endpoint")]
    Configuration,
}
impl RpcError {
    /// Stable wire-compatible error code for callers to branch on.
    pub fn protocol_code(&self) -> &str {
        match self {
            Self::Wire(error) => error.code.as_str(),
            Self::Timeout => "CLIENT_REQUEST_TIMEOUT",
            Self::Disconnected | Self::Transport(_) => "CLIENT_DISCONNECTED",
            Self::TooLarge => "TRANSFER_TOO_LARGE",
            Self::ChannelUnavailable(_) => "CHANNEL_NOT_FOUND",
            Self::Decode(_) => "CLIENT_DECODE_ERROR",
            Self::Configuration => "CLIENT_CONFIGURATION_ERROR",
        }
    }
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
        generation: u64,
        switch_to: Option<String>,
    },
}
/// Cloneable local RPC handle; no frontend thread performs socket IO.
#[derive(Clone)]
pub struct RpcClient {
    commands: async_channel::Sender<Command>,
    shared: Arc<Shared>,
    timeout: Duration,
    stop: async_channel::Sender<()>,
    endpoints: async_channel::Sender<Endpoint>,
    retry: async_channel::Sender<()>,
    bound_generation: Option<u64>,
}
impl RpcClient {
    /// Return immediately and connect on an independent smol IO thread.
    pub fn connect(options: ClientOptions) -> Self {
        let (commands, receiver) = async_channel::unbounded();
        let (stop, stopping) = async_channel::bounded(1);
        let (endpoints, updates) = async_channel::unbounded();
        let (retry, retries) = async_channel::bounded(1);
        let shared = Arc::new(Shared::new(options.capabilities.clone()));
        let handle = Self {
            commands,
            shared: shared.clone(),
            timeout: options.request_timeout,
            stop,
            endpoints,
            retry,
            bound_generation: None,
        };
        let thread_shared = shared.clone();
        if std::thread::Builder::new()
            .name("agentkit-rpc".into())
            .spawn(move || {
                smol::block_on(transport::run(
                    options,
                    receiver,
                    thread_shared,
                    stopping,
                    updates,
                    retries,
                ))
            })
            .is_err()
        {
            locked(&shared.info).failure =
                Some(RpcError::Transport("Unable to start RPC thread".into()));
            shared.finish();
        }
        handle
    }
    /// Capture a connected handle and metadata atomically, before deferring an intent.
    /// Its RPCs fail with Disconnected if that socket generation is lost; they cannot
    /// queue on a replacement connection even when first polled after recovery.
    pub fn bind_connection(&self) -> Result<(Self, ConnectionSnapshot), RpcError> {
        let info = locked(&self.shared.info);
        if info.snapshot.state != ConnectionState::Connected
            || self.bound_generation.is_some_and(|g| g != info.generation)
        {
            return Err(RpcError::Disconnected);
        }
        let mut bound = self.clone();
        bound.bound_generation = Some(info.generation);
        Ok((bound, info.snapshot.clone()))
    }
    /// Ordinary RPC. No implicit retry, including for writes.
    pub async fn invoke<T: DeserializeOwned>(
        &self,
        channel: &str,
        args: Vec<Value>,
    ) -> Result<T, RpcError> {
        self.invoke_timeout(channel, args, self.timeout).await
    }
    /// Explicitly idempotent read; retries a timeout at most once. Never use for writes.
    pub async fn invoke_read<T: DeserializeOwned>(
        &self,
        channel: &str,
        args: Vec<Value>,
    ) -> Result<T, RpcError> {
        self.invoke_read_timeout(channel, args, self.timeout).await
    }
    /// Explicit idempotent read with a channel-specific per-attempt deadline.
    pub async fn invoke_read_timeout<T: DeserializeOwned>(
        &self,
        channel: &str,
        args: Vec<Value>,
        timeout: Duration,
    ) -> Result<T, RpcError> {
        let first = self.invoke_timeout(channel, args.clone(), timeout).await;
        if matches!(&first,Err(error) if matches!(error.protocol_code(),"CLIENT_REQUEST_TIMEOUT"|"REQUEST_TIMEOUT"))
        {
            self.invoke_timeout(channel, args, timeout).await
        } else {
            first
        }
    }
    /// RPC with a caller-selected deadline; queued stale writes are never replayed.
    pub async fn invoke_timeout<T: DeserializeOwned>(
        &self,
        channel: &str,
        args: Vec<Value>,
        timeout: Duration,
    ) -> Result<T, RpcError> {
        self.call(channel, args, timeout, None).await
    }
    async fn call<T: DeserializeOwned>(
        &self,
        channel: &str,
        args: Vec<Value>,
        timeout: Duration,
        switch_to: Option<String>,
    ) -> Result<T, RpcError> {
        let frame = Envelope {
            id: uuid::Uuid::new_v4().to_string(),
            kind: EnvelopeKind::Request,
            channel: Some(channel.into()),
            args: Some(args),
            ..Envelope::default()
        };
        let (send, receive) = oneshot::channel();
        {
            // Sending under this lock makes queue invalidation atomic with a loss.
            let info = locked(&self.shared.info);
            if self.bound_generation.is_some_and(|g| {
                g != info.generation || info.snapshot.state != ConnectionState::Connected
            }) {
                return Err(RpcError::Disconnected);
            }
            if matches!(info.snapshot.state, ConnectionState::Fatal { .. }) {
                return Err(info.failure.clone().unwrap_or(RpcError::Disconnected));
            }
            if info.snapshot.state == ConnectionState::Disconnected || self.stop.is_full() {
                return Err(RpcError::Disconnected);
            }
            self.commands
                .try_send(Command::Invoke {
                    frame: Box::new(frame),
                    deadline: Instant::now() + timeout,
                    result: send,
                    generation: info.generation,
                    switch_to,
                })
                .map_err(|_| RpcError::Disconnected)?;
        }
        let value = smol::future::race(
            async { receive.await.map_err(|_| RpcError::Disconnected)? },
            async {
                smol::Timer::after(timeout).await;
                Err(RpcError::Timeout)
            },
        )
        .await?;
        Ok(serde_json::from_value(value)?)
    }
    /// Invoke with native binary leaves; base64 encoding counts toward the wire limit.
    pub async fn invoke_values(
        &self,
        channel: &str,
        args: Vec<agentkit_protocol::RpcValue>,
    ) -> Result<agentkit_protocol::RpcValue, RpcError> {
        let args = args
            .into_iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()?;
        self.invoke(channel, args).await
    }
    fn mapped_subscription<T: Send + 'static>(
        &self,
        channel: &str,
        map: impl Fn(Vec<Value>) -> Result<T, RpcError> + Send + 'static,
    ) -> async_channel::Receiver<Result<T, RpcError>> {
        let raw = self.subscribe(channel);
        let (send, receive) = async_channel::unbounded();
        smol::spawn(async move {
            loop {
                let next = smol::future::race(async { raw.recv().await.ok() }, async {
                    send.closed().await;
                    None
                })
                .await;
                let Some(args) = next else {
                    break;
                };
                if send.send(map(args)).await.is_err() {
                    break;
                }
            }
        })
        .detach();
        receive
    }
    /// Binary-aware event arguments. Dropping the receiver releases its forwarding task.
    pub fn subscribe_values(
        &self,
        channel: &str,
    ) -> async_channel::Receiver<Result<Vec<agentkit_protocol::RpcValue>, RpcError>> {
        self.mapped_subscription(channel, |args| {
            Ok(serde_json::from_value(Value::Array(args))?)
        })
    }
    /// Validated session events, preserving future tags and reporting decode failures.
    pub fn session_events(
        &self,
    ) -> async_channel::Receiver<Result<agentkit_protocol::DecodedSessionEvent, RpcError>> {
        self.mapped_subscription(
            agentkit_protocol::constants::channels::sessions::EVENT,
            |args| {
                Ok(serde_json::from_value(
                    args.into_iter().next().unwrap_or(Value::Null),
                )?)
            },
        )
    }
    /// Register/replace a reverse handler. New names are advertised on the next handshake;
    /// use ClientOptions::capabilities for initial registration. Browser stays unavailable.
    pub fn handle_capability(&self, name: &'static str, handler: CapabilityHandler) {
        if capability::permitted(name) {
            locked(&self.shared.capabilities).insert(name.into(), handler);
        }
    }
    /// Subscribe with an independent unbounded queue, retained across reconnections.
    /// `__transport:reconnected` emits one bool: true requires a full cache reload.
    pub fn subscribe(&self, channel: &str) -> async_channel::Receiver<Vec<Value>> {
        let (send, receive) = async_channel::unbounded();
        let info = locked(&self.shared.info);
        if !info.finished {
            locked(&self.shared.subscribers)
                .entry(channel.into())
                .or_default()
                .push(send);
        }
        receive
    }
    /// Current lifecycle snapshot.
    pub fn state(&self) -> ConnectionState {
        locked(&self.shared.info).snapshot.state.clone()
    }
    /// Lifecycle stream, beginning with the current state.
    pub fn state_changes(&self) -> async_channel::Receiver<ConnectionState> {
        self.shared.state_changes()
    }
    /// Atomic metadata for the latest connection.
    pub fn snapshot(&self) -> ConnectionSnapshot {
        locked(&self.shared.info).snapshot.clone()
    }
    /// Last connection error, cleared by a successful handshake.
    pub fn last_error(&self) -> Option<RpcError> {
        locked(&self.shared.info).failure.clone()
    }
    /// Whether a channel is advertised; legacy servers without a list are permissive.
    pub fn has_channel(&self, channel: &str) -> bool {
        locked(&self.shared.info)
            .snapshot
            .registered_channels
            .as_ref()
            .is_none_or(|c| c.iter().any(|c| c == channel))
    }
    /// Backend version from the latest successful handshake.
    pub fn server_version(&self) -> Option<String> {
        self.snapshot().server_version
    }
    /// Change server binding, and update future handshakes only after its RPC succeeds.
    pub async fn switch_workspace(&self, id: &str) -> Result<(), RpcError> {
        self.switch_workspace_timeout(id, self.timeout).await
    }
    /// Switch binding with a facade-selected deadline, updating binding on success.
    pub async fn switch_workspace_timeout(
        &self,
        id: &str,
        timeout: Duration,
    ) -> Result<(), RpcError> {
        let _: Value = self
            .call(
                agentkit_protocol::constants::channels::window::SWITCH_WORKSPACE,
                vec![serde_json::json!(id)],
                timeout,
                Some(id.into()),
            )
            .await?;
        Ok(())
    }
    /// Replace token/address after sidecar restart; never reuse the old reconnect cursor.
    pub fn replace_endpoint(&self, endpoint: Endpoint) {
        let _ = self.endpoints.try_send(endpoint);
    }
    /// Wake a scheduled reconnect without discarding the server replay cursor.
    pub fn retry_now(&self) {
        if matches!(self.state(), ConnectionState::Reconnecting { .. }) {
            let _ = self.retry.try_send(());
        }
    }
    /// Stop retries and reject outstanding requests. Shared by every clone.
    pub fn close(&self) {
        let _ = self.stop.try_send(());
    }
}

/// Typed subset used by the phase-zero native workbench.
pub mod api;
