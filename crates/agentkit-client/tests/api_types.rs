#![allow(clippy::unwrap_used)]
use agentkit_client::api::secrets::{
    ChatGptFlow, ClaudeExchange, ConnectionSetup, TestConnectionParams,
};
use secrecy::ExposeSecret;
use serde_json::json;
#[test]
fn credential_requests_serialize_but_debug_is_redacted_and_null_is_distinct_from_omission() {
    let setup = ConnectionSetup {
        slug: "test".into(),
        credential: Some("private-key".into()),
        base_url: Some(None),
        ..Default::default()
    };
    let wire = serde_json::to_value(&setup).unwrap();
    assert_eq!(wire["credential"], "private-key");
    assert_eq!(wire["baseUrl"], json!(null));
    assert!(wire.get("defaultModel").is_none());
    assert!(!format!("{setup:?}").contains("private-key"));
    let test = TestConnectionParams::new("another-key".into());
    assert_eq!(serde_json::to_value(&test).unwrap()["provider"], "pi");
    assert!(!format!("{test:?}").contains("another-key"));
    let result: ClaudeExchange =
        serde_json::from_value(json!({"success":true,"token":"private-token"})).unwrap();
    assert_eq!(
        result.token.as_ref().unwrap().expose_secret(),
        "private-token"
    );
    assert!(!format!("{result:?}").contains("private-token"));
    let flow:ChatGptFlow=serde_json::from_value(json!({"authUrl":"https://example.test/?state=private-state","state":"private-state","flowId":"private-flow"})).unwrap();
    assert!(!format!("{flow:?}").contains("private-state"));
}
#[test]
fn send_ack_cannot_default_missing_or_rejected_acceptance_to_success() {
    use agentkit_client::api::types::SendAck;
    assert!(serde_json::from_value::<SendAck>(json!({"messageId":"m"})).is_err());
    assert!(serde_json::from_value::<SendAck>(json!({"accepted":false,"messageId":"m"})).is_err());
}
#[test]
fn required_nullable_permission_config_accepts_null_but_not_absence() {
    use agentkit_client::api::types::DefaultPermissions;
    assert!(
        serde_json::from_value::<DefaultPermissions>(json!({"config":null,"path":"p"})).is_ok()
    );
    assert!(serde_json::from_value::<DefaultPermissions>(json!({"path":"p"})).is_err());
}
#[test]
fn generated_surface_cannot_publish_plaintext_secret_fields() {
    let generated = include_str!("../../agentkit-protocol/src/generated.rs");
    for field in ["pub credential:", "pub api_key:", "pub token:"] {
        assert!(!generated.contains(field), "{field}");
    }
}
