#![allow(clippy::unwrap_used)]
use agentkit_sidecar::{SidecarConfig, SidecarError, SidecarEvent, SidecarHandle};
use secrecy::ExposeSecret;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
fn fixture(mode: &str) -> (tempfile::TempDir, SidecarConfig) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("backend");
    let runtime = root
        .join("vendor/bun")
        .join(if cfg!(windows) { "bun.exe" } else { "bun" });
    std::fs::create_dir_all(runtime.parent().unwrap()).unwrap();
    let backend = PathBuf::from(std::env::var_os("AGENTKIT_BACKEND_DIR").unwrap());
    let source = backend
        .join("vendor/bun")
        .join(runtime.file_name().unwrap());
    if std::fs::hard_link(&source, &runtime).is_err() {
        std::fs::copy(source, &runtime).unwrap();
    }
    std::fs::write(
        root.join("server.js"),
        include_str!("../../../tools/sidecar-fixture/server.js"),
    )
    .unwrap();
    let mut cfg = SidecarConfig::new(root, temp.path().join("data"));
    cfg.extra_env.push(("FIXTURE_MODE".into(), mode.into()));
    cfg.ready_timeout = Duration::from_secs(3);
    (temp, cfg)
}
async fn event(events: &async_channel::Receiver<SidecarEvent>) -> SidecarEvent {
    smol::future::race(async { events.recv().await.unwrap() }, async {
        smol::Timer::after(Duration::from_secs(10)).await;
        panic!("sidecar event deadline")
    })
    .await
}
fn crash(data: &Path) {
    std::fs::write(data.join("crash"), "1").unwrap();
}
#[test]
#[ignore = "Requires packaged Bun; AGENTKIT_BACKEND_DIR"]
fn crashes_restart_with_fresh_tokens_then_stop_at_rolling_budget() {
    let (_root, config) = fixture("ready");
    let data = config.config_dir.clone();
    let log = config.log_dir.join("sidecar.log");
    smol::block_on(async {
        let sidecar = SidecarHandle::start(config).await.unwrap();
        let events = sidecar.events();
        let mut endpoint = sidecar.endpoint();
        let mut tokens = vec![endpoint.token.expose_secret().to_owned()];
        for index in 0..4 {
            let started = Instant::now();
            crash(&data);
            assert!(matches!(
                event(&events).await,
                SidecarEvent::Exited { code: Some(17), .. }
            ));
            match event(&events).await {
                SidecarEvent::Restarted { endpoint: next } if index < 3 => {
                    assert!(started.elapsed() >= Duration::from_millis(950));
                    assert_ne!(endpoint.url, next.url);
                    assert_ne!(endpoint.token.expose_secret(), next.token.expose_secret());
                    tokens.push(next.token.expose_secret().to_owned());
                    endpoint = next;
                }
                SidecarEvent::Fatal {
                    reason: SidecarError::RestartLimit,
                } if index == 3 => {}
                other => panic!("unexpected lifecycle event {other:?}"),
            }
        }
        smol::Timer::after(Duration::from_millis(1100)).await;
        assert_eq!(std::fs::read_to_string(data.join("attempts")).unwrap(), "4");
        sidecar.shutdown(Duration::from_millis(100)).await.unwrap();
        let log = std::fs::read_to_string(log).unwrap();
        for token in tokens {
            assert!(!log.contains(&token));
        }
        assert!(!log.contains("MKAGENT_SERVER_TOKEN="));
    });
}
#[test]
#[ignore = "Requires packaged Bun; AGENTKIT_BACKEND_DIR"]
fn occupied_and_timeout_do_not_retry_and_manual_restart_is_not_a_crash() {
    let (_temp, config) = fixture("occupied");
    let data = config.config_dir.clone();
    assert!(matches!(
        smol::block_on(SidecarHandle::start(config)),
        Err(SidecarError::AlreadyRunning { pid: Some(12345) })
    ));
    assert_eq!(std::fs::read_to_string(data.join("attempts")).unwrap(), "1");
    let (_temp, mut config) = fixture("hang");
    config.ready_timeout = Duration::from_millis(100);
    assert!(matches!(
        smol::block_on(SidecarHandle::start(config)),
        Err(SidecarError::ReadyTimeout)
    ));
    let (_temp, mut config) = fixture("ready");
    config.max_restarts = 0;
    smol::block_on(async {
        let sidecar = SidecarHandle::start(config).await.unwrap();
        let endpoint = sidecar.endpoint();
        let next = sidecar.restart().await.unwrap();
        assert_ne!(endpoint.token.expose_secret(), next.token.expose_secret());
        sidecar.shutdown(Duration::from_millis(100)).await.unwrap();
    });
}

async fn wait_file(path: &Path) {
    let until = Instant::now() + Duration::from_secs(5);
    while !path.is_file() {
        assert!(Instant::now() < until, "fixture file deadline");
        smol::Timer::after(Duration::from_millis(10)).await;
    }
}
async fn heartbeat_stops(path: &Path) {
    smol::Timer::after(Duration::from_millis(150)).await;
    let before = std::fs::read_to_string(path).unwrap();
    smol::Timer::after(Duration::from_millis(150)).await;
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        before,
        "descendant survived process-tree cleanup"
    );
}
#[test]
#[ignore = "Requires packaged Bun; AGENTKIT_BACKEND_DIR"]
fn shutdown_cancels_backoff_kills_descendants_and_protects_environment() {
    let (_temp, mut config) = fixture("descendant");
    let data = config.config_dir.clone();
    config.extra_env.extend([
        ("MKAGENT_RPC_HOST".into(), "0.0.0.0".into()),
        ("MKAGENT_RPC_PORT".into(), "9999".into()),
        ("MKAGENT_SERVER_TOKEN".into(), "not-allowed".into()),
        ("MKAGENT_WEBUI_DIR".into(), "unwanted".into()),
        ("MKAGENT_RPC_TLS_TEST".into(), "unwanted".into()),
        ("FIXTURE_CUSTOM".into(), "applied".into()),
        ("FIXTURE_LOG_OCCUPIED".into(), "1".into()),
        ("FIXTURE_API_KEY".into(), "custom-private-value".into()),
    ]);
    assert!(!format!("{config:?}").contains("custom-private-value"));
    smol::block_on(async {
        let sidecar = SidecarHandle::start(config).await.unwrap();
        let events = sidecar.events();
        let other = sidecar.events();
        assert_ne!(sidecar.endpoint().token.expose_secret(), "not-allowed");
        wait_file(&data.join("heartbeat")).await;
        let env: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(data.join("environment.json")).unwrap())
                .unwrap();
        assert_eq!(env["host"], "127.0.0.1");
        assert_eq!(env["port"], "0");
        assert!(env["webui"].is_null());
        assert!(env["tls"].is_null());
        assert_eq!(env["custom"], "applied");
        crash(&data);
        assert!(matches!(event(&events).await, SidecarEvent::Exited { .. }));
        assert!(matches!(event(&other).await, SidecarEvent::Exited { .. }));
        sidecar.shutdown(Duration::from_millis(100)).await.unwrap();
        heartbeat_stops(&data.join("heartbeat")).await;
        smol::Timer::after(Duration::from_millis(1000)).await;
        assert_eq!(std::fs::read_to_string(data.join("attempts")).unwrap(), "1");
        assert!(
            !std::fs::read_to_string(data.join("logs/sidecar.log"))
                .unwrap()
                .contains("custom-private-value")
        );
    });
}

#[cfg(windows)]
#[test]
#[ignore = "Internal subprocess entry for the parent-death test"]
fn owned_supervisor_fixture() {
    let Some(root) = std::env::var_os("SHIPAGENT_TEST_SUPERVISOR_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let mut config = SidecarConfig::new(root.join("backend"), root.join("data"));
    config
        .extra_env
        .push(("FIXTURE_MODE".into(), "descendant".into()));
    smol::block_on(async {
        let _handle = SidecarHandle::start(config).await.unwrap();
        loop {
            smol::Timer::after(Duration::from_secs(60)).await;
        }
    });
}
#[cfg(windows)]
#[test]
#[ignore = "Requires packaged Bun; verifies real Job Object parent-death cleanup"]
fn job_object_kills_process_tree_when_supervisor_is_abruptly_terminated() {
    struct Owner(std::process::Child);
    impl Drop for Owner {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let (root, config) = fixture("descendant");
    let data = config.config_dir;
    let mut owner = Owner(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "owned_supervisor_fixture", "--ignored"])
            .env("SHIPAGENT_TEST_SUPERVISOR_ROOT", root.path())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap(),
    );
    smol::block_on(async {
        wait_file(&data.join("heartbeat")).await;
        owner.0.kill().unwrap();
        owner.0.wait().unwrap();
        heartbeat_stops(&data.join("heartbeat")).await;
    });
}

#[test]
#[ignore = "Requires actual bundled backend; AGENTKIT_BACKEND_DIR"]
fn real_backend_lock_conflict_and_crash_recovery_replace_rpc_endpoint() {
    use agentkit_client::{ClientOptions, RpcClient};
    let data = tempfile::tempdir().unwrap();
    let config = SidecarConfig::new(
        PathBuf::from(std::env::var_os("AGENTKIT_BACKEND_DIR").unwrap()),
        data.path().into(),
    );
    smol::block_on(async {
        let sidecar = SidecarHandle::start(config.clone()).await.unwrap();
        let endpoint = sidecar.endpoint();
        let events = sidecar.events();
        let lock_path = data.path().join(".server.lock");
        let record: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&lock_path).unwrap()).unwrap();
        let pid = u32::try_from(record["pid"].as_u64().unwrap()).unwrap();
        assert!(
            matches!(SidecarHandle::start(config).await, Err(SidecarError::AlreadyRunning { pid: Some(owner) }) if owner == pid)
        );
        let client = RpcClient::connect(ClientOptions::local(
            endpoint.url.clone(),
            endpoint.token.clone(),
            String::new(),
        ));
        client.service().health().await.unwrap();
        #[cfg(unix)]
        assert!(
            std::process::Command::new("/bin/kill")
                .args(["-KILL", &pid.to_string()])
                .status()
                .unwrap()
                .success()
        );
        #[cfg(windows)]
        assert!(
            std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .stdout(std::process::Stdio::null())
                .status()
                .unwrap()
                .success()
        );
        assert!(matches!(event(&events).await, SidecarEvent::Exited { .. }));
        let SidecarEvent::Restarted { endpoint: next } = event(&events).await else {
            panic!("backend did not restart")
        };
        assert_ne!(endpoint.token.expose_secret(), next.token.expose_secret());
        let previous = client.snapshot().recovery_id;
        client.replace_endpoint(next);
        let until = Instant::now() + Duration::from_secs(10);
        while client.snapshot().recovery_id <= previous
            || client.state() != agentkit_client::ConnectionState::Connected
        {
            assert!(Instant::now() < until, "replacement handshake deadline");
            smol::Timer::after(Duration::from_millis(10)).await;
        }
        client.service().health().await.unwrap();
        assert!(!client.workspaces().list().await.unwrap().is_empty());
        assert!(
            client.snapshot().needs_refresh,
            "new backend identity must require cache reload"
        );
        assert_eq!(
            client.snapshot().refresh_epoch,
            client.snapshot().recovery_id
        );
        client.close();
        sidecar.shutdown(Duration::from_secs(5)).await.unwrap();
        #[cfg(unix)]
        assert!(
            !lock_path.exists(),
            "graceful shutdown failed to release lock"
        );
    });
}

#[test]
#[ignore = "Requires packaged Bun; validates cancellation while awaiting initial readiness"]
fn cancelling_startup_cleans_up_the_unreturned_process() {
    let (_temp, mut config) = fixture("hang");
    config.ready_timeout = Duration::from_secs(15);
    let data = config.config_dir.clone();
    let runtime = config
        .backend_dir
        .join("vendor/bun")
        .join(if cfg!(windows) { "bun.exe" } else { "bun" });
    smol::block_on(async {
        smol::future::race(
            async {
                let _ = SidecarHandle::start(config).await;
                panic!("hang fixture became ready")
            },
            wait_file(&data.join("pid")),
        )
        .await;
        let pid = std::fs::read_to_string(data.join("pid")).unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let code = std::process::Command::new(&runtime).args(["-e", "try { process.kill(Number(process.argv[1]), 0); process.exit(1) } catch(e) { process.exit(e.code === 'ESRCH' ? 0 : 2) }", &pid])
                .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().unwrap();
            if code.success() {
                break;
            }
            assert!(
                Instant::now() < until,
                "cancelled startup left a running process"
            );
            smol::Timer::after(Duration::from_millis(50)).await;
        }
    });
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "Internal subprocess entry for macOS login-shell probes"]
fn shell_probe_fixture() {
    let Some(root) = std::env::var_os("SHIPAGENT_TEST_SHELL_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let mut config = SidecarConfig::new(root.join("backend"), root.join("data"));
    config
        .extra_env
        .push(("FIXTURE_MODE".into(), "ready".into()));
    smol::block_on(async {
        let handle = SidecarHandle::start(config).await.unwrap();
        handle.restart().await.unwrap();
        handle.shutdown(Duration::from_millis(100)).await.unwrap();
    });
}
#[cfg(target_os = "macos")]
#[test]
#[ignore = "Requires packaged Bun; macOS login-shell import/cache/five-second fallback"]
fn macos_imports_and_caches_login_shell_and_bounds_a_hung_shell() {
    use std::os::unix::fs::PermissionsExt;
    for hang in [false, true] {
        let (root, _) = fixture("ready");
        let shell = root.path().join("login-shell");
        let calls = root.path().join("shell-calls");
        let quoted = calls.to_string_lossy().replace('\'', "'\"'\"'");
        let body = if hang {
            "while :; do /bin/sleep 1; done"
        } else {
            "printf '__SHIPAGENT_ENV__\\0FIXTURE_CUSTOM=shell-loaded\\0PATH=/usr/bin:/bin\\0'"
        };
        std::fs::write(
            &shell,
            format!("#!/bin/sh\nprintf x >> '{quoted}'\n{body}\n"),
        )
        .unwrap();
        std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o700)).unwrap();
        let started = Instant::now();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "shell_probe_fixture", "--ignored"])
            .env("SHELL", shell)
            .env("SHIPAGENT_TEST_SHELL_ROOT", root.path())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(
            std::fs::read_to_string(calls).unwrap(),
            "x",
            "shell result was not cached across restarts"
        );
        if hang {
            assert!(started.elapsed() >= Duration::from_secs(5));
            assert!(started.elapsed() < Duration::from_secs(12));
        } else {
            let env: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(root.path().join("data/environment.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(env["custom"], "shell-loaded");
        }
    }
}

#[cfg(unix)]
#[test]
#[ignore = "Requires packaged Bun; a surviving descendant must receive its shutdown grace"]
fn descendant_can_finish_term_cleanup_after_the_leader_exits() {
    let (_temp, config) = fixture("graceful");
    let data = config.config_dir.clone();
    smol::block_on(async {
        let handle = SidecarHandle::start(config).await.unwrap();
        wait_file(&data.join("heartbeat")).await;
        handle.shutdown(Duration::from_secs(1)).await.unwrap();
        assert_eq!(
            std::fs::read_to_string(data.join("child-flushed")).unwrap(),
            "yes"
        );
    });
}
