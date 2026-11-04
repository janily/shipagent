#![allow(clippy::unwrap_used)]
//! First-run deferral and tested configuration against the fixed backend.
use agentkit_client::api::{
    WorkbenchApi,
    secrets::{ConnectionSetup, TestConnectionParams},
};
use agentkit_protocol::generated as g;
use agentkit_sidecar::{SidecarConfig, SidecarHandle};
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    time::Duration,
};
struct Model(Child);
impl Drop for Model {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[test]
#[ignore = "Requires AGENTKIT_BACKEND_DIR and Node"]
fn first_run_defer_test_save_default_and_restart() {
    let temp = tempfile::tempdir().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut model = Model(
        Command::new("node")
            .arg(root.join("tools/settings/model.mjs"))
            .env("AGENTKIT_FIXTURE_ROOT", temp.path())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut line = String::new();
    BufReader::new(model.0.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let url = line
        .trim()
        .strip_prefix("FAKE_LLM_URL=")
        .unwrap()
        .to_string();
    smol::block_on(async {
        let sidecar = SidecarHandle::start(SidecarConfig::new(
            std::env::var_os("AGENTKIT_BACKEND_DIR").unwrap().into(),
            temp.path().join("config"),
        ))
        .await
        .unwrap();
        let api = WorkbenchApi::connect(sidecar.endpoint(), String::new());
        api.health().await.unwrap();
        assert!(api.connections_generated().await.unwrap().is_empty());
        api.disable_browser().await.unwrap();
        api.defer_setup().await.unwrap();
        let config: serde_json::Value =
            serde_json::from_slice(&std::fs::read(temp.path().join("config/config.json")).unwrap())
                .unwrap();
        assert_eq!(config["setupDeferred"], true);
        assert_eq!(config["browserToolEnabled"], false);
        let mut params = TestConnectionParams::new("".into());
        params.base_url = Some(url.clone());
        params.pi_auth_provider = Some("openai".into());
        params.model = Some("fixture-model".into());
        params.custom_endpoint = Some(g::CustomEndpointConfig {
            api: g::CustomEndpointConfigApi::OpenaiCompletions,
            supports_images: Some(true),
            max_tokens: Some(4096.),
        });
        let before = api.connections_generated().await.unwrap().len();
        api.test_connection_draft(&params).await.unwrap();
        assert_eq!(
            api.connections_generated().await.unwrap().len(),
            before,
            "testing an unsaved form must not register it"
        );
        let record:g::LlmConnection=serde_json::from_value(serde_json::json!({"slug":"pi-api-key","name":"Native local","providerType":"pi_compat","authType":"none","createdAt":42.,"baseUrl":url,"defaultModel":"fixture-model","models":["fixture-model"],"customEndpoint":{"api":"openai-completions","supportsImages":true,"maxTokens":4096}})).unwrap();
        let setup = ConnectionSetup {
            slug: record.slug.clone(),
            base_url: Some(record.base_url.clone()),
            default_model: Some(record.default_model.clone()),
            models: Some(Some(vec!["fixture-model".into()])),
            custom_endpoint: record.custom_endpoint.clone(),
            ..Default::default()
        };
        api.save_connection(&setup, &record).await.unwrap();
        api.test_connection(&record.slug).await.unwrap();
        api.default_connection(&record.slug).await.unwrap();
        api.refresh_connection_models(&record.slug).await.unwrap();
        let saved = api
            .connections_generated()
            .await
            .unwrap()
            .into_iter()
            .find(|c| c.slug == record.slug)
            .unwrap();
        assert_eq!(saved.name, "Native local");
        assert_eq!(saved.created_at, 42.);
        assert!(saved.is_authenticated);
        assert_eq!(saved.is_default, Some(true));
        assert_eq!(
            saved.custom_endpoint.as_ref().unwrap().max_tokens,
            Some(4096.)
        );
        let mut edit = record.clone();
        edit.name = "Renamed local".into();
        let edit_setup = ConnectionSetup {
            update_only: Some(true),
            ..setup
        };
        api.save_connection(&edit_setup, &edit).await.unwrap();
        assert_eq!(
            api.connections_generated()
                .await
                .unwrap()
                .into_iter()
                .find(|c| c.slug == edit.slug)
                .unwrap()
                .name,
            "Renamed local"
        );
        let config: serde_json::Value =
            serde_json::from_slice(&std::fs::read(temp.path().join("config/config.json")).unwrap())
                .unwrap();
        assert_ne!(
            config["setupDeferred"], true,
            "saving clears backend deferral"
        );
        api.close();
        sidecar.shutdown(Duration::from_secs(5)).await.unwrap();
        let sidecar = SidecarHandle::start(SidecarConfig::new(
            std::env::var_os("AGENTKIT_BACKEND_DIR").unwrap().into(),
            temp.path().join("config"),
        ))
        .await
        .unwrap();
        let api = WorkbenchApi::connect(sidecar.endpoint(), String::new());
        api.health().await.unwrap();
        let connections = api.connections_generated().await.unwrap();
        assert!(
            connections.iter().any(|c| c.slug == "pi-api-key"
                && c.is_authenticated
                && c.is_default == Some(true))
        );
        api.close();
        sidecar.shutdown(Duration::from_secs(5)).await.unwrap();
    });
}
