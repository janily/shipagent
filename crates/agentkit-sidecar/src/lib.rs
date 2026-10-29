//! Nonblocking local backend startup and explicit shutdown prototype (T-0-03).
//! Automatic restart, persistent logs and Windows Job Objects follow T-1-05/T-3-01.
#![deny(missing_docs)]
use agentkit_protocol::Endpoint;
use rand::RngCore;
use secrecy::ExposeSecret;
use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// Headless backend launch configuration.
#[derive(Clone, Debug)]
pub struct SidecarConfig {
    /// Directory containing server.js, vendor/bun and resources.
    pub backend_dir: PathBuf,
    /// Isolated CONFIG_DIR, never shared with another running backend.
    pub config_dir: PathBuf,
    /// Readiness deadline, default 15 seconds.
    pub ready_timeout: Duration,
    /// Application version sent to the backend.
    pub app_version: String,
}
impl SidecarConfig {
    /// Configure a local sidecar with documented startup defaults.
    pub fn new(backend_dir: PathBuf, config_dir: PathBuf) -> Self {
        Self {
            backend_dir,
            config_dir,
            ready_timeout: Duration::from_secs(15),
            app_version: env!("CARGO_PKG_VERSION").into(),
        }
    }
}
/// Startup or shutdown failure. Errors never include raw backend output.
#[derive(Debug, thiserror::Error)]
pub enum SidecarError {
    /// Runtime or entry point does not exist.
    #[error("Missing backend artifact: {0}")]
    MissingRuntime(PathBuf),
    /// Process or pipe operation failed.
    #[error("Backend process operation failed: {0}")]
    Io(#[from] std::io::Error),
    /// No readiness line within the configured deadline.
    #[error("Backend readiness timed out")]
    ReadyTimeout,
    /// Backend exited before becoming ready.
    #[error("Backend exited before becoming ready")]
    ExitedEarly,
    /// Another backend holds this configuration directory's lock.
    #[error("Another backend is using this data directory")]
    AlreadyRunning,
    /// A readiness line did not contain a safe local endpoint.
    #[error("Invalid backend readiness endpoint")]
    InvalidEndpoint,
}
/// Interpret a readiness line; ordinary output and token lines are discarded.
/// Only a loopback, nonzero-port ws URL without embedded credentials is accepted.
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
/// Owns a backend process. All startup and shutdown IO runs off the GUI thread.
pub struct SidecarHandle {
    child: Option<Child>,
    endpoint: Endpoint,
}
impl SidecarHandle {
    /// Start and await readiness on a background worker.
    pub async fn start(config: SidecarConfig) -> Result<Self, SidecarError> {
        smol::unblock(move || spawn(config)).await
    }
    /// Obtain the memory-only loopback endpoint and redacted token.
    pub fn endpoint(&self) -> Endpoint {
        self.endpoint.clone()
    }
    /// Terminate the Unix process group, allowing graceful shutdown before killing it.
    /// Windows prototype terminates its direct child; Job Object coverage is not yet implemented.
    pub async fn shutdown(mut self, grace: Duration) -> Result<(), SidecarError> {
        if let Some(child) = self.child.take() {
            smol::unblock(move || terminate(child, grace)).await?;
        }
        Ok(())
    }
}
impl Drop for SidecarHandle {
    fn drop(&mut self) {
        if let Some(child) = self.child.take() {
            // Never block a GUI thread when a handle is abandoned.
            let _ = std::thread::Builder::new()
                .name("agentkit-backend-cleanup".into())
                .spawn(move || {
                    let _ = terminate(child, Duration::from_secs(1));
                });
        }
    }
}
fn spawn(config: SidecarConfig) -> Result<SidecarHandle, SidecarError> {
    let root = config
        .backend_dir
        .canonicalize()
        .map_err(|_| SidecarError::MissingRuntime(config.backend_dir.clone()))?;
    let runtime = root
        .join("vendor/bun")
        .join(if cfg!(windows) { "bun.exe" } else { "bun" });
    for path in [&runtime, &root.join("server.js")] {
        if !path.is_file() {
            return Err(SidecarError::MissingRuntime(path.clone()));
        }
    }
    std::fs::create_dir_all(&config.config_dir)?;
    let config_dir = config.config_dir.canonicalize()?;
    let mut random = [0u8; 32];
    rand::rng().fill_bytes(&mut random);
    let token: secrecy::SecretString = random
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        .into();
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    };
    let mut paths = vec![
        root.join("resources/bin"),
        root.join(format!("resources/bin/{platform}-{arch}")),
        root.join("vendor/bun"),
    ];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let mut command = Command::new(runtime);
    command
        .arg(root.join("server.js"))
        .current_dir(&root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("CONFIG_DIR", config_dir)
        .env("MKAGENT_SERVER_TOKEN", token.expose_secret())
        .env("MKAGENT_RPC_HOST", "127.0.0.1")
        .env("MKAGENT_RPC_PORT", "0")
        .env("MKAGENT_VERSION", config.app_version)
        .env("MKAGENT_IS_PACKAGED", "true")
        .env("MKAGENT_BUNDLED_ASSETS_ROOT", &root)
        .env("MKAGENT_APP_ROOT", &root)
        .env("MKAGENT_RESOURCES_PATH", root.join("resources"))
        .env(
            "MKAGENT_UV",
            root.join(format!(
                "resources/bin/{platform}-{arch}/{}",
                if cfg!(windows) { "uv.exe" } else { "uv" }
            )),
        )
        .env("MKAGENT_SCRIPTS", root.join("resources/scripts"))
        .env(
            "PATH",
            std::env::join_paths(paths).map_err(std::io::Error::other)?,
        );
    for name in [
        "MKAGENT_WEBUI_DIR",
        "MKAGENT_RPC_TLS_CERT",
        "MKAGENT_RPC_TLS_KEY",
        "MKAGENT_RPC_TLS_CA",
        "MKAGENT_HEALTH_PORT",
    ] {
        command.env_remove(name);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("Missing backend stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| std::io::Error::other("Missing backend stderr"))?;
    let occupied = Arc::new(AtomicBool::new(false));
    let occupied_worker = occupied.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if line.contains("Another server instance is already running") {
                occupied_worker.store(true, Ordering::Relaxed);
            }
        }
    });
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut sent = false;
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if !sent {
                match parse_ready_line(&line) {
                    Ok(None) => {}
                    result => {
                        let _ = send.send(result);
                        sent = true;
                    }
                }
            }
            // Drain the pipe without forwarding raw text or authentication lines to logs.
        }
    });
    match receive.recv_timeout(config.ready_timeout) {
        Ok(Ok(Some(url))) => Ok(SidecarHandle {
            child: Some(child),
            endpoint: Endpoint { url, token },
        }),
        result => {
            let _ = terminate(child, Duration::from_millis(200));
            if occupied.load(Ordering::Relaxed) {
                return Err(SidecarError::AlreadyRunning);
            }
            match result {
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(SidecarError::ReadyTimeout),
                Ok(Err(error)) => Err(error),
                _ => Err(SidecarError::ExitedEarly),
            }
        }
    }
}
fn terminate(mut child: Child, grace: Duration) -> Result<(), SidecarError> {
    #[cfg(unix)]
    {
        let _ = Command::new("/bin/kill")
            .args(["-TERM", "--", &format!("-{}", child.id())])
            .status();
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
    let deadline = Instant::now() + grace;
    while Instant::now() < deadline {
        if child.try_wait()?.is_some() {
            #[cfg(unix)]
            {
                let _ = Command::new("/bin/kill")
                    .args(["-KILL", "--", &format!("-{}", child.id())])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    #[cfg(unix)]
    {
        let _ = Command::new("/bin/kill")
            .args(["-KILL", "--", &format!("-{}", child.id())])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    child.wait()?;
    Ok(())
}
