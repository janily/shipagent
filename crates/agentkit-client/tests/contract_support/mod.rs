#![allow(clippy::unwrap_used)]
use agentkit_client::{ClientOptions, RpcClient};
use agentkit_protocol::Endpoint;
use futures::StreamExt;
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    time::Duration,
};

pub async fn deadline<T>(future: impl std::future::Future<Output = T>) -> T {
    smol::future::race(future, async {
        smol::Timer::after(Duration::from_secs(30)).await;
        panic!("real contract deadline exceeded")
    })
    .await
}
pub async fn until(mut predicate: impl FnMut() -> bool) {
    deadline(async {
        while !predicate() {
            smol::Timer::after(Duration::from_millis(10)).await;
        }
    })
    .await
}
pub struct Fixture {
    child: Child,
    _profile: tempfile::TempDir,
    pub endpoint: Endpoint,
}
impl Fixture {
    pub fn start() -> Self {
        let backend = std::path::PathBuf::from(
            std::env::var_os("AGENTKIT_BACKEND_DIR").expect("set AGENTKIT_BACKEND_DIR"),
        );
        let script = std::env::var_os("AGENTKIT_CONTRACT_SERVER")
            .expect("set AGENTKIT_CONTRACT_SERVER; build with --contract");
        let profile = tempfile::tempdir().unwrap();
        let token = uuid::Uuid::new_v4().to_string();
        let runtime =
            backend
                .join("vendor/bun")
                .join(if cfg!(windows) { "bun.exe" } else { "bun" });
        let mut child = Command::new(runtime)
            .arg(script)
            .env("SHIPAGENT_CONTRACT_TOKEN", &token)
            .env("CONFIG_DIR", profile.path())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            // Drain output through process lifetime; only readiness leaves this thread.
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(url) = line.strip_prefix("CONTRACT_URL=") {
                    let _ = tx.send(url.to_owned());
                }
            }
        });
        // Install cleanup before any readiness assertion can panic.
        let mut fixture = Self {
            child,
            _profile: profile,
            endpoint: Endpoint {
                url: String::new(),
                token: token.into(),
            },
        };
        fixture.endpoint.url = rx
            .recv_timeout(Duration::from_secs(15))
            .expect("contract fixture did not become ready");
        fixture
    }
    pub fn options(&self, workspace: &str) -> ClientOptions {
        let mut options = ClientOptions::local(
            self.endpoint.url.clone(),
            self.endpoint.token.clone(),
            workspace.into(),
        );
        options.max_reconnect_delay = Duration::from_millis(25);
        options
    }
    pub fn client(&self, workspace: &str) -> RpcClient {
        RpcClient::connect(self.options(workspace))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Clone, Copy)]
enum Control {
    Pause,
    Resume,
    Stop,
}
/// A frame-transparent bridge: only closes sockets and gates the next accept.
/// Never modifies handshakes, responses, events or buffer state.
pub struct FaultProxy {
    pub url: String,
    control: async_channel::Sender<Control>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl FaultProxy {
    pub fn start(upstream: String) -> Self {
        let (control, commands) = async_channel::unbounded();
        let (urls, recv) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            smol::block_on(async move {
                let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                urls.send(format!("ws://{}", listener.local_addr().unwrap()))
                    .unwrap();
                loop {
                    let accepted =
                        smol::future::race(async { Some(listener.accept().await) }, async {
                            let _ = commands.recv().await;
                            None
                        })
                        .await;
                    let Some(Ok((tcp, _))) = accepted else { break };
                    let upstream_url = upstream.clone();
                    let setup = async move {
                        let downstream = async_tungstenite::accept_async(tcp).await.unwrap();
                        let url = url::Url::parse(&upstream_url).unwrap();
                        let tcp = async_net::TcpStream::connect(("127.0.0.1", url.port().unwrap()))
                            .await
                            .unwrap();
                        let (upstream, _) = async_tungstenite::client_async(&upstream_url, tcp)
                            .await
                            .unwrap();
                        (downstream, upstream)
                    };
                    let connected =
                        smol::future::race(async { Some(deadline(setup).await) }, async {
                            let _ = commands.recv().await;
                            None
                        })
                        .await;
                    let Some((downstream, upstream)) = connected else {
                        break;
                    };
                    let forward = async move {
                        let (mut out_up, mut in_up) = upstream.split();
                        let (mut out_down, mut in_down) = downstream.split();
                        smol::future::race(
                            async {
                                while let Some(Ok(frame)) = in_down.next().await {
                                    if out_up.send(frame).await.is_err() {
                                        break;
                                    }
                                }
                            },
                            async {
                                while let Some(Ok(frame)) = in_up.next().await {
                                    if out_down.send(frame).await.is_err() {
                                        break;
                                    }
                                }
                            },
                        )
                        .await;
                        Control::Stop
                    };
                    let command = smol::future::race(forward, async {
                        commands.recv().await.unwrap_or(Control::Stop)
                    })
                    .await;
                    if !matches!(command, Control::Pause) {
                        break;
                    }
                    // Both sockets are dropped before waiting, so upstream observes a real disconnect.
                    if !matches!(commands.recv().await, Ok(Control::Resume)) {
                        break;
                    }
                }
            })
        });
        Self {
            url: recv.recv_timeout(Duration::from_secs(5)).unwrap(),
            control,
            worker: Some(worker),
        }
    }
    pub async fn pause(&self) {
        self.control.send(Control::Pause).await.unwrap();
    }
    pub async fn resume(&self) {
        self.control.send(Control::Resume).await.unwrap();
    }
}
impl Drop for FaultProxy {
    fn drop(&mut self) {
        let _ = self.control.try_send(Control::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[test]
fn stalled_proxy_handshake_cannot_block_cleanup() {
    use std::io::Write;
    let proxy = FaultProxy::start("ws://127.0.0.1:9".into());
    let url = url::Url::parse(&proxy.url).unwrap();
    let mut stalled = std::net::TcpStream::connect(("127.0.0.1", url.port().unwrap())).unwrap();
    stalled.write_all(b"GET / HTTP/1.1\r\n").unwrap();
    // Let the worker read the deliberately incomplete upgrade; no external service needed.
    std::thread::sleep(Duration::from_millis(50));
    let (done, waiting) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        drop(proxy);
        let _ = done.send(());
    });
    assert!(
        waiting.recv_timeout(Duration::from_secs(2)).is_ok(),
        "stalled handshake blocked Drop"
    );
    drop(stalled);
}
