use crate::{SidecarConfig, SidecarError, lock, output::Output, process::Tree};
use agentkit_protocol::Endpoint;
use rand::RngCore;
use secrecy::{ExposeSecret, SecretString};
use std::{
    process::{Command, Stdio},
    sync::{Arc, mpsc},
    thread::JoinHandle,
    time::{Duration, Instant},
};
pub(crate) struct Process {
    pub tree: Tree,
    pub endpoint: Endpoint,
    pub output: Arc<Output>,
    pub ready: mpsc::Receiver<Result<String, SidecarError>>,
    pub deadline: Instant,
    readers: Vec<JoinHandle<()>>,
}
impl Process {
    pub fn start(
        config: &SidecarConfig,
        stop: &std::sync::atomic::AtomicBool,
    ) -> Result<Self, SidecarError> {
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
        let data = config.config_dir.canonicalize()?;
        let mut bytes = [0; 32];
        rand::rng().fill_bytes(&mut bytes);
        let token: SecretString = bytes
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
            .into();
        let mut secrets = vec![token.clone()];
        secrets.extend(
            config
                .extra_env
                .iter()
                .filter(|(key, _)| {
                    let key = key.to_ascii_uppercase();
                    key.contains("TOKEN")
                        || key.contains("KEY")
                        || key.contains("PASS")
                        || key.contains("PROXY")
                })
                .map(|(_, value)| value.clone().into()),
        );
        let output = Output::new(&config.log_dir, secrets)?;
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
        let mut command = Command::new(runtime);
        command
            .arg(root.join("server.js"))
            .current_dir(&root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        crate::environment::apply(&mut command, &config.extra_env, &root, platform, arch, stop)?;
        if stop.load(std::sync::atomic::Ordering::Acquire) {
            return Err(SidecarError::Stopped);
        }
        command
            .env("CONFIG_DIR", data)
            .env("MKAGENT_SERVER_TOKEN", token.expose_secret())
            .env("MKAGENT_RPC_HOST", "127.0.0.1")
            .env("MKAGENT_RPC_PORT", "0")
            .env("MKAGENT_VERSION", &config.app_version)
            .env("MKAGENT_IS_PACKAGED", "true")
            .env("MKAGENT_BUNDLED_ASSETS_ROOT", &root)
            .env("MKAGENT_APP_ROOT", &root)
            .env("MKAGENT_RESOURCES_PATH", root.join("resources"))
            .env("MKAGENT_SCRIPTS", root.join("resources/scripts"))
            .env(
                "MKAGENT_UV",
                root.join(format!(
                    "resources/bin/{platform}-{arch}/{}",
                    if cfg!(windows) { "uv.exe" } else { "uv" }
                )),
            );
        let tree = Tree::spawn(command)?;
        let (send, ready) = mpsc::channel();
        let mut process = Self {
            tree,
            endpoint: Endpoint {
                url: String::new(),
                token,
            },
            output,
            ready,
            deadline: Instant::now() + config.ready_timeout,
            readers: vec![],
        };
        let stdout = process
            .tree
            .inner()
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::other("Missing stdout"))?;
        let stderr = process
            .tree
            .inner()
            .stderr
            .take()
            .ok_or_else(|| std::io::Error::other("Missing stderr"))?;
        let output = process.output.clone();
        let ready_send = send.clone();
        process.readers.push(
            std::thread::Builder::new()
                .name("agentkit-sidecar-stdout".into())
                .spawn(move || output.drain(stdout, false, ready_send))?,
        );
        let output = process.output.clone();
        process.readers.push(
            std::thread::Builder::new()
                .name("agentkit-sidecar-stderr".into())
                .spawn(move || output.drain(stderr, true, send))?,
        );
        Ok(process)
    }
    pub fn finish_for_restart(
        &mut self,
        stop: &std::sync::atomic::AtomicBool,
    ) -> Result<(), SidecarError> {
        self.finish_inner(Duration::from_secs(5), Some(stop))
    }
    pub fn finish(&mut self, grace: Duration) -> Result<(), SidecarError> {
        self.finish_inner(grace, None)
    }
    fn finish_inner(
        &mut self,
        grace: Duration,
        stop: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<(), SidecarError> {
        self.tree.terminate(grace, stop)?;
        let until = Instant::now() + Duration::from_millis(250);
        for reader in self.readers.drain(..) {
            while !reader.is_finished() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(2));
            }
            if reader.is_finished() {
                let _ = reader.join();
            } else {
                tracing::warn!(
                    "Sidecar pipe remains open outside the owned process tree; detaching bounded reader"
                );
            }
        }
        Ok(())
    }
    pub fn startup_error(&self, code: Option<i32>) -> SidecarError {
        if let Some(pid) = *lock(&self.output.occupied) {
            SidecarError::AlreadyRunning { pid }
        } else {
            SidecarError::ExitedEarly {
                code,
                stderr_tail: lock(&self.output.tail).clone(),
            }
        }
    }
}
