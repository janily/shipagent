#![allow(clippy::unwrap_used)]
//! Fixed backend preparation and clear contract; no provider token request.
use agentkit_client::api::WorkbenchApi;
use agentkit_sidecar::{SidecarConfig, SidecarHandle};
use secrecy::ExposeSecret;
#[test]
#[ignore = "Requires bundled backend; never exchanges a live provider code"]
fn claude_prepares_pkce_and_clear_prevents_code_exchange() {
    smol::block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let backend = SidecarHandle::start(SidecarConfig::new(
            std::env::var_os("AGENTKIT_BACKEND_DIR").unwrap().into(),
            temp.path().join("config"),
        ))
        .await
        .unwrap();
        let api = WorkbenchApi::connect(backend.endpoint(), String::new());
        api.health().await.unwrap();
        let before = api.connections_generated().await.unwrap().len();
        let flow = api.start_claude().await.unwrap();
        assert!(flow.success);
        let u = url::Url::parse(flow.auth_url.as_ref().unwrap().expose_secret()).unwrap();
        assert_eq!(u.host_str(), Some("claude.ai"));
        assert!(
            u.query_pairs()
                .any(|(k, v)| k == "code_challenge_method" && v == "S256")
        );
        assert!(u.query_pairs().any(|(k, v)| k == "redirect_uri"
            && v == "https://console.anthropic.com/oauth/code/callback"));
        api.clear_claude().await.unwrap();
        assert!(
            api.finish_claude("claude-max-999", &"fixture-never-sent-to-provider".into())
                .await
                .is_err()
        );
        assert_eq!(api.connections_generated().await.unwrap().len(), before);
        api.discard_claude("claude-max-999").await;
        backend
            .shutdown(std::time::Duration::from_secs(3))
            .await
            .unwrap();
    });
}
