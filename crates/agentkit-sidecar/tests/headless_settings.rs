#![allow(clippy::unwrap_used)]
//! T-2-01: real authenticated RPC, persisted restart acknowledgement and live routing.
use agentkit_client::api::{
    WorkbenchApi,
    secrets::{ConnectionSetup, NetworkProxy},
};
use agentkit_protocol::generated as g;
use agentkit_sidecar::{SidecarConfig, SidecarHandle};
use secrecy::{ExposeSecret, SecretString};
use std::{
    io::{BufRead, BufReader},
    path::Path,
    process::{Child, Command, Stdio},
    time::Duration,
};
struct Fixture(Child);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn counts(path: &Path) -> [u64; 3] {
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    [
        v["origin"].as_u64().unwrap(),
        v["a"].as_u64().unwrap(),
        v["b"].as_u64().unwrap(),
    ]
}
async fn restart(sidecar: &SidecarHandle, api: &WorkbenchApi) {
    let old = sidecar.endpoint();
    let epoch = api.snapshot().recovery_id;
    let next = sidecar.request_restart().await.unwrap();
    assert_ne!(old.token.expose_secret(), next.token.expose_secret());
    api.replace_endpoint(next);
    let until = std::time::Instant::now() + Duration::from_secs(10);
    while {
        let snapshot = api.snapshot();
        snapshot.state != agentkit_client::ConnectionState::Connected
            || snapshot.recovery_id <= epoch
    } {
        assert!(
            std::time::Instant::now() < until,
            "new endpoint handshake deadline"
        );
        smol::Timer::after(Duration::from_millis(10)).await;
    }
    api.health().await.unwrap();
}
#[test]
#[ignore = "Requires bundled backend and Node; only local proof endpoints are used"]
fn proxy_and_power_preferences_survive_restart_and_only_restart_changes_routing() {
    let temp = tempfile::tempdir().unwrap();
    let stat = temp.path().join("counts.json");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut fixture = Fixture(
        Command::new("node")
            .arg(root.join("tools/headless-proxy/fixture.mjs"))
            .arg(&stat)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut line = String::new();
    BufReader::new(fixture.0.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let urls: serde_json::Value = serde_json::from_str(&line).unwrap();
    let origin = urls["origin"].as_str().unwrap().to_owned();
    let a = urls["a"].as_str().unwrap().to_owned();
    let b = urls["b"].as_str().unwrap().to_owned();
    smol::block_on(async {
        let mut config = SidecarConfig::new(
            std::env::var_os("AGENTKIT_BACKEND_DIR").unwrap().into(),
            temp.path().join("config"),
        );
        config.extra_env.extend(
            [
                "HTTP_PROXY",
                "HTTPS_PROXY",
                "NO_PROXY",
                "ALL_PROXY",
                "http_proxy",
                "https_proxy",
                "no_proxy",
                "all_proxy",
            ]
            .map(|key| (key.into(), String::new())),
        );
        config.extra_env.extend([
            ("HTTP_PROXY".into(), a.clone()),
            ("http_proxy".into(), a.clone()),
            ("HTTPS_PROXY".into(), b.clone()),
            ("https_proxy".into(), b.clone()),
        ]);
        let sidecar = SidecarHandle::start(config).await.unwrap();
        let api = WorkbenchApi::connect(sidecar.endpoint(), String::new());
        api.health().await.unwrap();
        let record:g::LlmConnection=serde_json::from_value(serde_json::json!({"slug":"headless-proof","name":"Local proof","providerType":"pi","authType":"api_key","createdAt":1,"piAuthProvider":"anthropic","baseUrl":origin,"defaultModel":"claude-sonnet-4-6","models":["claude-sonnet-4-6"]})).unwrap();
        let setup = ConnectionSetup {
            slug: record.slug.clone(),
            credential: Some("sk-ant-fixture-only".into()),
            pi_auth_provider: Some("anthropic".into()),
            models: Some(Some(vec!["claude-sonnet-4-6".into()])),
            base_url: Some(Some(origin.clone())),
            default_model: Some(Some("claude-sonnet-4-6".into())),
            ..Default::default()
        };
        api.save_connection(&setup, &record).await.unwrap();
        api.test_connection(&record.slug).await.unwrap();
        assert_eq!(counts(&stat), [1, 0, 0]);
        let mut proxy = NetworkProxy {
            enabled: true,
            http_proxy: Some(SecretString::from(a.clone())),
            https_proxy: Some(SecretString::from(b.clone())),
            no_proxy: None,
        };
        assert!(
            api.save_network_proxy(&proxy)
                .await
                .unwrap()
                .restart_required
        );
        api.set_keep_awake(true).await.unwrap();
        assert!(api.keep_awake().await.unwrap());
        api.test_connection(&record.slug).await.unwrap();
        assert_eq!(
            counts(&stat),
            [2, 0, 0],
            "save must not change live routing"
        );
        restart(&sidecar, &api).await;
        assert!(api.keep_awake().await.unwrap());
        let saved = api.network_proxy().await.unwrap();
        assert!(saved.enabled);
        assert!(saved.http_proxy.as_ref().unwrap().expose_secret() == a);
        api.test_connection(&record.slug).await.unwrap();
        assert_eq!(counts(&stat), [2, 1, 0], "HTTP must select the HTTP proxy");
        proxy.http_proxy = Some(SecretString::from(b.clone()));
        assert!(
            api.save_network_proxy(&proxy)
                .await
                .unwrap()
                .restart_required
        );
        api.test_connection(&record.slug).await.unwrap();
        assert_eq!(counts(&stat), [2, 2, 0]);
        restart(&sidecar, &api).await;
        api.test_connection(&record.slug).await.unwrap();
        assert_eq!(counts(&stat), [2, 2, 1]);
        proxy.no_proxy = Some(format!(
            "127.0.0.1:{}",
            url::Url::parse(&origin).unwrap().port().unwrap()
        ));
        api.save_network_proxy(&proxy).await.unwrap();
        restart(&sidecar, &api).await;
        api.test_connection(&record.slug).await.unwrap();
        assert_eq!(
            counts(&stat),
            [3, 2, 1],
            "NO_PROXY must bypass both proxies"
        );
        api.save_network_proxy(&NetworkProxy::default())
            .await
            .unwrap();
        api.set_keep_awake(false).await.unwrap();
        restart(&sidecar, &api).await;
        assert!(!api.keep_awake().await.unwrap());
        assert!(!api.network_proxy().await.unwrap().enabled);
        api.test_connection(&record.slug).await.unwrap();
        assert_eq!(counts(&stat), [4, 2, 1]);
        api.close();
        sidecar.shutdown(Duration::from_secs(2)).await.unwrap();
    });
}
