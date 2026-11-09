#![allow(clippy::unwrap_used)]
//! Real backend PKCE preparation, client ownership and cancellation without provider access.
use agentkit_client::api::{
    WorkbenchApi,
    secrets::{ChatGptCancel, ChatGptComplete},
};
use agentkit_sidecar::{SidecarConfig, SidecarHandle};
use secrecy::ExposeSecret;
#[test]
#[ignore = "Requires bundled backend; no external token exchange or user account"]
fn prepared_flow_is_owned_and_cancellation_prevents_exchange() {
    smol::block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let backend = SidecarHandle::start(SidecarConfig::new(
            std::env::var_os("AGENTKIT_BACKEND_DIR").unwrap().into(),
            temp.path().join("config"),
        ))
        .await
        .unwrap();
        let owner = WorkbenchApi::connect(backend.endpoint(), String::new());
        owner.health().await.unwrap();
        let before = owner.connections_generated().await.unwrap().len();
        let flow = owner.start_chatgpt("chatgpt-plus-123456789").await.unwrap();
        let u = url::Url::parse(flow.auth_url.expose_secret()).unwrap();
        assert_eq!(u.host_str(), Some("auth.openai.com"));
        assert!(
            u.query_pairs()
                .any(|(k, v)| k == "state" && v == flow.state.expose_secret())
        );
        assert!(
            u.query_pairs()
                .any(|(k, v)| k == "code_challenge_method" && v == "S256")
        );
        let rival = WorkbenchApi::connect(backend.endpoint(), String::new());
        rival.health().await.unwrap();
        let args = ChatGptComplete {
            flow_id: flow.flow_id,
            code: "not-a-provider-code".into(),
            state: flow.state.clone(),
        };
        assert!(rival.complete_chatgpt(&args).await.is_err());
        owner
            .cancel_chatgpt(Some(&ChatGptCancel {
                state: Some(flow.state),
            }))
            .await
            .unwrap();
        assert!(owner.complete_chatgpt(&args).await.is_err());
        assert_eq!(owner.connections_generated().await.unwrap().len(), before);
        backend
            .shutdown(std::time::Duration::from_secs(3))
            .await
            .unwrap();
    });
}
