#![allow(clippy::unwrap_used)]
use agentkit_protocol::generated::{Message, Session, SessionEvent};
use serde_json::{Value, json};
fn decode_event(value: Value) -> Result<SessionEvent, serde_json::Error> {
    serde_json::from_value(value)
}
#[test]
fn event_discriminators_and_both_retry_shapes_are_precise() {
    for value in [
        json!({"type":"text_delta","sessionId":"s","delta":"中文","futureField":true}),
        json!({"type":"retry","sessionId":"s","phase":"backoff","message":"wait"}),
        json!({"type":"retry","sessionId":"s","phase":"active"}),
        json!({"type":"retry","sessionId":"s","phase":"end"}),
        json!({"type":"permission_request","sessionId":"s","request":{"sessionId":"s","requestId":"r","toolName":"bash","description":"run?","canAlwaysAllow":false}}),
    ] {
        let parsed = decode_event(value.clone()).unwrap();
        let wire = serde_json::to_value(parsed).unwrap();
        assert_eq!(wire["type"], value["type"]);
        assert_eq!(wire["phase"], value["phase"]);
    }
    assert!(decode_event(json!({"type":"retry","sessionId":"s","phase":"backoff"})).is_err());
    assert!(decode_event(json!({"type":"retry","sessionId":"s","phase":"invalid"})).is_err());
    assert!(decode_event(json!({"type":"text_delta","sessionId":"s"})).is_err());
    assert!(decode_event(json!({"type":"text_delta","sessionId":"s","delta":42})).is_err());
}
#[test]
fn message_keeps_optional_values_and_required_fields_are_not_defaulted() {
    let wire = json!({"id":"m","role":"tool","content":"read","timestamp":123,"toolName":"Read","toolUseId":"call","toolInput":{"path":"proof.txt"},"toolStatus":"completed","toolResult":"proof","isError":false,"hidden":true,"futureField":{"x":1}});
    let parsed: Message = serde_json::from_value(wire.clone()).unwrap();
    let encoded = serde_json::to_value(parsed).unwrap();
    for key in [
        "id",
        "role",
        "content",
        "toolInput",
        "toolStatus",
        "isError",
        "hidden",
    ] {
        assert_eq!(encoded[key], wire[key]);
    }
    assert_eq!(encoded["timestamp"].as_f64(), Some(123.0));
    assert!(
        serde_json::from_value::<Message>(
            json!({"id":"m","role":"assistant","content":"missing timestamp"})
        )
        .is_err()
    );
}
#[test]
fn session_restores_nested_messages_and_ignores_added_fields() {
    let mut wire = json!({"id":"s","workspaceId":"w","workspaceName":"工作区","lastMessageAt":123,"messages":[{"id":"m","role":"assistant","content":"answer","timestamp":123}],"isProcessing":false,"permissionMode":"allow-all","futureFlag":true});
    let parsed: Session = serde_json::from_value(wire.clone()).unwrap();
    let encoded = serde_json::to_value(parsed).unwrap();
    assert_eq!(encoded["messages"][0]["content"], "answer");
    assert_eq!(encoded["permissionMode"], "allow-all");
    wire["permissionMode"] = json!("ALLOW_ALL");
    assert!(serde_json::from_value::<Session>(wire).is_err());
}

#[test]
fn unknown_events_round_trip_but_known_malformed_events_are_rejected() {
    use agentkit_protocol::DecodedSessionEvent;
    let future = json!({"type":"future_event","sessionId":"s","payload":{"nested":[1,2]}});
    let event: DecodedSessionEvent = serde_json::from_value(future.clone()).unwrap();
    assert!(event.known().is_none());
    assert_eq!(serde_json::to_value(event).unwrap(), future);
    let known = json!({"type":"complete","sessionId":"s","futureField":true});
    let event: DecodedSessionEvent = serde_json::from_value(known.clone()).unwrap();
    assert!(event.known().is_some());
    assert_eq!(serde_json::to_value(event).unwrap(), known);
    for malformed in [
        json!({"type":"text_delta","sessionId":"s"}),
        json!({"type":"retry","sessionId":"s","phase":"bad"}),
        json!({"type":"future_event"}),
        json!({"type":10,"sessionId":"s"}),
    ] {
        assert!(serde_json::from_value::<DecodedSessionEvent>(malformed).is_err());
    }
}

#[test]
fn optional_collections_preserve_absent_versus_explicit_empty() {
    let base = json!({"id":"m","role":"user","content":"hi","timestamp":123});
    let absent: Message = serde_json::from_value(base.clone()).unwrap();
    assert!(
        serde_json::to_value(absent)
            .unwrap()
            .get("attachments")
            .is_none()
    );
    let mut with_empty = base;
    with_empty["attachments"] = json!([]);
    with_empty["toolInput"] = json!({});
    let present: Message = serde_json::from_value(with_empty).unwrap();
    let encoded = serde_json::to_value(present).unwrap();
    assert_eq!(encoded.get("attachments"), Some(&json!([])));
    assert_eq!(encoded.get("toolInput"), Some(&json!({})));
}

#[test]
fn required_nullable_fields_accept_null_but_reject_absence() {
    assert!(
        decode_event(json!({"type":"session_model_changed","sessionId":"s","model":null})).is_ok()
    );
    assert!(decode_event(json!({"type":"session_model_changed","sessionId":"s"})).is_err());
    let mut context = json!({"usedTokens":null,"limitKind":"context","isEstimate":false,"isStale":true,"canCompact":false});
    let valid = json!({"type":"usage_update","sessionId":"s","tokenUsage":{"inputTokens":1,"contextUsage":context.clone()}});
    assert!(decode_event(valid).is_ok());
    context.as_object_mut().unwrap().remove("usedTokens");
    let malformed = json!({"type":"usage_update","sessionId":"s","tokenUsage":{"inputTokens":1,"contextUsage":context}});
    assert!(decode_event(malformed.clone()).is_err());
    assert!(serde_json::from_value::<agentkit_protocol::DecodedSessionEvent>(malformed).is_err());
}
