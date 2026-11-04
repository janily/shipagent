#![allow(clippy::unwrap_used)]
use super::*;
fn values(kind: ConnectionKind) -> ConnectionValues {
    ConnectionValues {
        slug: "local".into(),
        name: "Local".into(),
        kind,
        provider: "anthropic".into(),
        url: "http://127.0.0.1:11434/v1".into(),
        models: vec!["model".into()],
        model: "model".into(),
        credential: None,
        existing: false,
    }
}
#[::core::prelude::v1::test]
fn custom_protocol_selects_its_own_provider_hint_and_rejects_embedded_credentials() {
    let mut v = values(ConnectionKind::OpenAi);
    let record = connection_record(&v, None).unwrap();
    assert_eq!(record.pi_auth_provider.as_deref(), Some("openai"));
    assert_eq!(
        test_params(&v, &record).pi_auth_provider.as_deref(),
        Some("openai")
    );
    v.kind = ConnectionKind::Anthropic;
    assert_eq!(
        connection_record(&v, None)
            .unwrap()
            .pi_auth_provider
            .as_deref(),
        Some("anthropic")
    );
    v.url = "https://user:fixture-key@example.com/v1".into();
    assert!(connection_record(&v, None).is_err());
    v.url = "file:///tmp/model".into();
    assert!(connection_record(&v, None).is_err());
}
#[::core::prelude::v1::test]
fn new_preset_requires_a_key_but_existing_edits_keep_the_backend_reference() {
    let mut v = values(ConnectionKind::Preset);
    assert!(connection_record(&v, None).is_err());
    v.credential = Some("fixture-only".into());
    assert!(connection_record(&v, None).is_ok());
    v.existing = true;
    v.credential = None;
    assert!(
        setup(&v, &connection_record(&v, None).unwrap())
            .credential
            .is_none()
    );
}
#[::core::prelude::v1::test]
fn editing_retains_model_hints_identity_and_endpoint_options() {
    let old: g::LlmConnectionWithStatus=serde_json::from_value(serde_json::json!({"slug":"local","name":"Old","providerType":"pi_compat","authType":"none","createdAt":42,"isAuthenticated":true,"models":[{"id":"model","name":"Model","shortName":"Model","description":"Local model","provider":"pi","contextWindow":32768,"supportsImages":false}],"defaultModel":"model","customEndpoint":{"api":"openai-completions","maxTokens":4096,"supportsImages":true}})).unwrap();
    let mut v = values(ConnectionKind::Ollama);
    v.existing = true;
    let record = connection_record(&v, Some(&old)).unwrap();
    assert_eq!(record.created_at, 42.);
    assert_eq!(
        record.custom_endpoint.as_ref().unwrap().max_tokens,
        Some(4096.)
    );
    let raw = serde_json::to_value(record.models).unwrap();
    assert_eq!(raw[0]["contextWindow"], 32768.);
    assert_eq!(raw[0]["supportsImages"], false);
}
