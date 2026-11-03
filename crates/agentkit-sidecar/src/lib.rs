//! Nonblocking local backend supervision, bounded diagnostics and process-tree ownership.
#![deny(missing_docs)]
mod actor;
mod environment;
mod launch;
mod output;
mod process;
use agentkit_protocol::Endpoint;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

/// Headless backend launch configuration. Reserved transport variables are always owned by the supervisor.
#[derive(Clone)]
pub struct SidecarConfig {
    /// Directory containing server.js, vendor/bun and resources.
    pub backend_dir: PathBuf,
    /// Isolated CONFIG_DIR, never shared with another running backend.
    pub config_dir: PathBuf,
    /// Diagnostic directory, defaults to CONFIG_DIR/logs.
    pub log_dir: PathBuf,
    /// Readiness deadline, default 15 seconds.
    pub ready_timeout: Duration,
    /// Application version sent to the backend.
    pub app_version: String,
    /// Additional environment, applied before protected launch variables; never included in Debug.
    pub extra_env: Vec<(String, String)>,
    /// Allowed unexpected exits in a rolling 60-second window, default three.
    pub max_restarts: u32,
}
impl std::fmt::Debug for SidecarConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SidecarConfig")
            .field("backend_dir", &self.backend_dir)
            .field("config_dir", &self.config_dir)
            .field("extra_env", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}
impl SidecarConfig {
    /// Configure documented local-backend defaults.
    pub fn new(backend_dir: PathBuf, config_dir: PathBuf) -> Self {
        Self {
            backend_dir,
            log_dir: config_dir.join("logs"),
            config_dir,
            ready_timeout: Duration::from_secs(15),
            app_version: env!("CARGO_PKG_VERSION").into(),
            extra_env: vec![],
            max_restarts: 3,
        }
    }
}
/// Startup, supervision or shutdown failure. Display never includes captured backend output.
#[derive(Clone, Debug, thiserror::Error)]
pub enum SidecarError {
    /// Runtime or entry point does not exist.
    #[error("Missing backend artifact: {0}")]
    MissingRuntime(PathBuf),
    /// Process or pipe operation failed.
    #[error("Backend process operation failed: {0}")]
    Io(Arc<std::io::Error>),
    /// No readiness line within the configured deadline.
    #[error("Backend readiness timed out")]
    ReadyTimeout,
    /// Backend exited before becoming ready; tail is bounded and sanitized.
    #[error("Backend exited before becoming ready")]
    ExitedEarly {
        /// OS exit code, absent after a signal.
        code: Option<i32>,
        /// Last 64KiB of sanitized stderr.
        stderr_tail: String,
    },
    /// Another backend holds the profile lock; never automatically retried.
    #[error("Another backend is using this data directory")]
    AlreadyRunning {
        /// Owner PID when reported by upstream; unknown is never invented as zero.
        pid: Option<u32>,
    },
    /// A readiness line did not contain a safe local endpoint.
    #[error("Invalid backend readiness endpoint")]
    InvalidEndpoint,
    /// More than the configured number of crashes occurred in 60 seconds.
    #[error("Backend restart limit exceeded")]
    RestartLimit,
    /// Supervisor was shut down or its handle was dropped.
    #[error("Backend supervisor stopped")]
    Stopped,
}
impl From<std::io::Error> for SidecarError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(Arc::new(value))
    }
}
/// Broadcast lifecycle events. Each subscription receives its own copy.
#[derive(Clone, Debug)]
pub enum SidecarEvent {
    /// An unexpected exit, emitted before retry/fatal classification.
    Exited {
        /// OS exit code, absent after a signal.
        code: Option<i32>,
        /// Last 64KiB of sanitized stderr.
        stderr_tail: String,
    },
    /// Automatic or explicit restart reached readiness with a fresh token.
    Restarted {
        /// Replace the client's endpoint; never replay pending writes.
        endpoint: Endpoint,
    },
    /// Supervision stopped retrying; explicit restart may be requested later.
    Fatal {
        /// Sanitized terminal error.
        reason: SidecarError,
    },
}
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
#[derive(Default)]
pub(crate) struct Bus {
    subscribers: Vec<async_channel::Sender<SidecarEvent>>,
    last: Option<SidecarEvent>,
}
pub(crate) struct Shared {
    stop: AtomicBool,
    endpoint: Mutex<Endpoint>,
    bus: Mutex<Bus>,
    error: Mutex<Option<SidecarError>>,
}
impl Shared {
    fn emit(&self, event: SidecarEvent) {
        let mut bus = lock(&self.bus);
        bus.last = Some(event.clone());
        bus.subscribers
            .retain(|send| send.try_send(event.clone()).is_ok());
    }
}
pub(crate) type Reply<T> = async_channel::Sender<Result<T, SidecarError>>;
pub(crate) enum Control {
    Restart(Reply<Endpoint>),
    Shutdown(Duration, Option<Reply<()>>),
}
/// One owner handle; all child IO and lifecycle transitions execute on a dedicated background thread.
pub struct SidecarHandle {
    send: std::sync::mpsc::Sender<Control>,
    shared: Arc<Shared>,
    closed: bool,
}
impl SidecarHandle {
    /// Launch the supervisor and await initial readiness without blocking the caller's thread.
    pub async fn start(config: SidecarConfig) -> Result<Self, SidecarError> {
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            endpoint: Mutex::new(Endpoint {
                url: String::new(),
                token: String::new().into(),
            }),
            bus: Mutex::new(Bus::default()),
            error: Mutex::new(None),
        });
        let (send, commands) = std::sync::mpsc::channel();
        let (ready, receiver) = async_channel::bounded(1);
        let worker_shared = shared.clone();
        let handle = Self {
            send,
            shared,
            closed: false,
        };
        std::thread::Builder::new()
            .name("agentkit-sidecar".into())
            .spawn(move || actor::run(config, commands, ready, worker_shared))?;
        receiver.recv().await.map_err(|_| SidecarError::Stopped)??;
        Ok(handle)
    }
    /// Last ready endpoint; may be disconnected during restart. Observe events for replacement.
    pub fn endpoint(&self) -> Endpoint {
        lock(&self.shared.endpoint).clone()
    }
    /// Independent event receiver, seeded with the latest event to avoid missing a fast exit.
    pub fn events(&self) -> async_channel::Receiver<SidecarEvent> {
        let (send, receive) = async_channel::unbounded();
        let mut bus = lock(&self.shared.bus);
        if let Some(event) = &bus.last {
            let _ = send.try_send(event.clone());
        }
        bus.subscribers.push(send);
        receive
    }
    /// Most recent terminal supervision error, cleared when explicit restart begins.
    pub fn last_error(&self) -> Option<SidecarError> {
        lock(&self.shared.error).clone()
    }
    /// Explicit restart. Healthy restarts do not consume the rolling crash budget.
    pub async fn restart(&self) -> Result<Endpoint, SidecarError> {
        let (send, receive) = async_channel::bounded(1);
        self.send
            .send(Control::Restart(send))
            .map_err(|_| SidecarError::Stopped)?;
        receive.recv().await.map_err(|_| SidecarError::Stopped)?
    }
    /// Stop supervising, cancel backoff and terminate/reap the current process tree.
    pub async fn shutdown(mut self, grace: Duration) -> Result<(), SidecarError> {
        let (send, receive) = async_channel::bounded(1);
        self.shared.stop.store(true, Ordering::Release);
        self.send
            .send(Control::Shutdown(grace, Some(send)))
            .map_err(|_| SidecarError::Stopped)?;
        self.closed = true;
        receive.recv().await.map_err(|_| SidecarError::Stopped)?
    }
}
impl Drop for SidecarHandle {
    fn drop(&mut self) {
        if !self.closed {
            self.shared.stop.store(true, Ordering::Release);
            let _ = self
                .send
                .send(Control::Shutdown(Duration::from_secs(1), None));
        }
    }
}
/// Parse only a loopback, nonzero-port readiness URL without credentials/query/fragment.
pub fn parse_ready_line(line: &str) -> Result<Option<String>, SidecarError> {
    let Some(raw) = line.strip_prefix("MKAGENT_SERVER_URL=") else {
        return Ok(None);
    };
    let url = url::Url::parse(raw).map_err(|_| SidecarError::InvalidEndpoint)?;
    if url.scheme() != "ws"
        || url.host_str() != Some("127.0.0.1")
        || url.port().is_none_or(|p| p == 0)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err(SidecarError::InvalidEndpoint);
    }
    Ok(Some(raw.into()))
}
