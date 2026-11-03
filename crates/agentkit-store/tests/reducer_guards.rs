#![allow(clippy::unwrap_used)]
use agentkit_protocol::{DecodedSessionEvent, generated as g};
use agentkit_store::core::reducer::{
    Context, ReducerError, SessionState, process, process_decoded,
};
use serde_json::json;
fn state() -> SessionState {
    serde_json::from_value(json!({"session":{"id":"s","workspaceId":"w","workspaceName":"W","lastMessageAt":1,"isProcessing":true,"messages":[]},"streaming":null})).unwrap()
}
fn context() -> Context {
    Context {
        now_ms: 50,
        message_id: "fresh".into(),
    }
}
#[test]
fn foreign_permission_requests_never_escape_as_current_session_effects() {
    let state = state();
    let e:g::SessionEvent=serde_json::from_value(json!({"type":"permission_request","sessionId":"other","request":{"sessionId":"other","requestId":"r","description":"run","toolName":"Bash"}})).unwrap();
    let result = process(
        &state,
        &e,
        &Context {
            now_ms: 0,
            message_id: String::new(),
        },
    )
    .unwrap();
    assert!(result.effects.is_empty());
    assert_eq!(
        serde_json::to_value(result.state).unwrap(),
        serde_json::to_value(state).unwrap()
    );
}
#[test]
fn future_tags_remain_at_ingress_while_reduction_is_a_noop() {
    let raw = json!({"type":"future_state","sessionId":"s","future":{"field":7}});
    let event: DecodedSessionEvent = serde_json::from_value(raw.clone()).unwrap();
    let result = process_decoded(&state(), &event, &context()).unwrap();
    assert!(event.known().is_none());
    assert_eq!(event.raw(), &raw);
    assert!(result.effects.is_empty());
    assert_eq!(
        serde_json::to_value(result.state).unwrap(),
        serde_json::to_value(state()).unwrap()
    );
}
#[test]
fn fractional_or_out_of_range_message_timestamps_fail_without_mutating_input() {
    for timestamp in [0.5, 9223372036854775808.0, -9223372036854777856.0] {
        let state = state();
        let before = serde_json::to_value(&state).unwrap();
        for mut raw in [
            json!({"type":"text_complete","sessionId":"s","text":"answer"}),
            json!({"type":"tool_start","sessionId":"s","toolName":"Read","toolUseId":"t","toolInput":{}}),
            json!({"type":"tool_result","sessionId":"s","toolName":"Read","toolUseId":"t","result":"done"}),
            json!({"type":"error","sessionId":"s","error":"offline"}),
            json!({"type":"info","sessionId":"s","message":"note"}),
            json!({"type":"typed_error","sessionId":"s","error":{"code":"network_error","title":"Network","message":"offline","canRetry":true,"actions":[]}}),
        ] {
            raw["timestamp"] = json!(timestamp);
            let event = serde_json::from_value(raw).unwrap();
            assert!(matches!(
                process(&state, &event, &context()),
                Err(ReducerError::InvalidTimestamp)
            ));
        }
        assert_eq!(serde_json::to_value(&state).unwrap(), before);
    }
}
#[test]
fn empty_id_context_is_reported_before_any_message_is_created() {
    let event =
        serde_json::from_value(json!({"type":"text_delta","sessionId":"s","delta":"hello"}))
            .unwrap();
    assert!(matches!(
        process(
            &state(),
            &event,
            &Context {
                now_ms: 1,
                message_id: String::new()
            }
        ),
        Err(ReducerError::EmptyMessageId)
    ));
}
#[test]
fn known_decoded_events_use_the_same_validated_dispatch() {
    let event: DecodedSessionEvent = serde_json::from_value(
        json!({"type":"text_delta","sessionId":"s","delta":"hello","extraFutureField":true}),
    )
    .unwrap();
    let result = process_decoded(&state(), &event, &context()).unwrap();
    assert_eq!(result.state.session.messages[0].content, "hello");
    assert_eq!(event.raw()["extraFutureField"], true);
}
