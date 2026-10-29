#![allow(clippy::unwrap_used)]
use agentkit_protocol::{Envelope, EnvelopeKind, ErrorCode, RpcValue};
use serde_json::json;

#[test]
fn handshake_keeps_camel_case_and_redacts_token() {
    let wire = json!({"id":"h1","type":"handshake","protocolVersion":"1.0","workspaceId":"ws","token":"private-token","clientCapabilities":[],"lastSeq":42});
    let frame: Envelope = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(frame.kind, EnvelopeKind::Handshake);
    assert_eq!(frame.last_seq, Some(42));
    assert!(!format!("{frame:?}").contains("private-token"));
    assert_eq!(serde_json::to_value(frame).unwrap(), wire);
}

#[test]
fn unknown_error_code_survives_round_trip() {
    let code: ErrorCode = serde_json::from_value(json!("FUTURE_CODE")).unwrap();
    assert_eq!(code.as_str(), "FUTURE_CODE");
    assert_eq!(serde_json::to_value(code).unwrap(), json!("FUTURE_CODE"));
}

#[test]
fn nested_binary_uses_upstream_wrapper() {
    let input = json!({"attachment":[{"__craftRpcType":"u8","base64":"AP+A"}],"text":"中文"});
    let decoded: RpcValue = serde_json::from_value(input.clone()).unwrap();
    let RpcValue::Object(object) = &decoded else {
        panic!("expected object")
    };
    let RpcValue::Array(values) = &object["attachment"] else {
        panic!("expected array")
    };
    assert_eq!(values[0], RpcValue::Bytes(vec![0, 255, 128]));
    assert_eq!(serde_json::to_value(decoded).unwrap(), input);
}

#[test]
fn invalid_binary_is_not_silently_accepted() {
    assert!(
        serde_json::from_value::<RpcValue>(json!({"__craftRpcType":"u8","base64":"not!base64"}))
            .is_err()
    );
}

#[test]
fn unknown_envelope_fields_are_compatible() {
    let frame: Envelope = serde_json::from_value(
        json!({"id":"1","type":"handshake_ack","clientId":"c","futureField":true}),
    )
    .unwrap();
    assert_eq!(frame.client_id.as_deref(), Some("c"));
    assert!(frame.token.is_none());
}

#[test]
fn null_response_result_is_not_lost() {
    let frame: Envelope =
        serde_json::from_value(json!({"id":"r1","type":"response","result":null})).unwrap();
    assert_eq!(
        serde_json::to_value(frame).unwrap(),
        json!({"id":"r1","type":"response","result":null})
    );
}
