#![allow(clippy::unwrap_used)]
use agentkit_sidecar::{SidecarConfig, SidecarError, SidecarHandle, parse_ready_line};
use std::time::Duration;
#[test]
fn readiness_only_accepts_local_nonzero_ports() {
    assert_eq!(
        parse_ready_line("MKAGENT_SERVER_URL=ws://127.0.0.1:32100").unwrap(),
        Some("ws://127.0.0.1:32100".into())
    );
    assert!(parse_ready_line("MKAGENT_SERVER_URL=ws://example.com:32100").is_err());
    assert!(parse_ready_line("MKAGENT_SERVER_URL=ws://127.0.0.1:0").is_err());
    assert!(parse_ready_line("MKAGENT_SERVER_URL=ws://127.0.0.1:32100/?token=secret").is_err());
    assert_eq!(
        parse_ready_line("MKAGENT_SERVER_TOKEN=private").unwrap(),
        None
    );
}
#[test]
fn missing_backend_runtime_is_reported_before_spawn() {
    let dir = tempfile::tempdir().unwrap();
    let config = SidecarConfig::new(dir.path().into(), dir.path().join("data"));
    assert!(matches!(
        smol::block_on(SidecarHandle::start(config)),
        Err(SidecarError::MissingRuntime(_))
    ));
}

#[test]
#[ignore = "Requires a built backend; run with AGENTKIT_BACKEND_DIR"]
fn bundled_backend_completes_real_rpc_handshake_and_shuts_down() {
    use agentkit_client::{ClientOptions, RpcClient};
    use secrecy::ExposeSecret;
    let backend = std::env::var_os("AGENTKIT_BACKEND_DIR").unwrap();
    let data = tempfile::tempdir().unwrap();
    smol::block_on(async {
        let sidecar = SidecarHandle::start(SidecarConfig::new(backend.into(), data.path().into()))
            .await
            .unwrap();
        let endpoint = sidecar.endpoint();
        assert_eq!(endpoint.token.expose_secret().len(), 64);
        assert!(!format!("{endpoint:?}").contains(endpoint.token.expose_secret()));
        let client = RpcClient::connect(ClientOptions::local(
            endpoint.url,
            endpoint.token,
            "".into(),
        ));
        let health: serde_json::Value = client.invoke("server:getHealth", vec![]).await.unwrap();
        assert!(health.get("status").is_some());
        let workspaces: serde_json::Value = client.invoke("workspaces:get", vec![]).await.unwrap();
        assert!(workspaces.as_array().is_some_and(|w| !w.is_empty()));
        client.close();
        sidecar.shutdown(Duration::from_secs(5)).await.unwrap();
    });
}
