#![allow(clippy::unwrap_used)]
use agentkit_protocol::prototype::{Session, SessionEvent};
use agentkit_store::{ChatState, RunState};
use serde_json::{Value, json};
fn state() -> ChatState {
    ChatState::new(Session {
        id: "s".into(),
        ..Session::default()
    })
}
fn apply(state: &mut ChatState, value: Value) {
    state.apply(&serde_json::from_value::<SessionEvent>(value).unwrap());
}
#[test]
fn streaming_replaces_optimistic_user_and_corrects_final_text() {
    let mut s = state();
    assert!(s.begin_send("hi".into(), "optimistic".into()));
    apply(
        &mut s,
        json!({"type":"user_message","sessionId":"s","optimisticMessageId":"optimistic","status":"accepted","message":{"id":"u1","role":"user","content":"hi"}}),
    );
    apply(
        &mut s,
        json!({"type":"text_delta","sessionId":"s","delta":"半","turnId":"t"}),
    );
    apply(
        &mut s,
        json!({"type":"text_delta","sessionId":"s","delta":"段","turnId":"t"}),
    );
    assert_eq!(s.session.messages.len(), 2);
    assert_eq!(s.session.messages[1].content, "半段");
    apply(
        &mut s,
        json!({"type":"text_complete","sessionId":"s","text":"完整答案","turnId":"t","messageId":"a1"}),
    );
    apply(&mut s, json!({"type":"complete","sessionId":"s"}));
    assert_eq!(s.status, RunState::Idle);
    assert_eq!(s.session.messages.len(), 2);
    assert_eq!(s.session.messages[1].id, "a1");
    assert_eq!(s.session.messages[1].content, "完整答案");
}
#[test]
fn tool_results_update_the_same_nested_call() {
    let mut s = state();
    s.begin_send("read".into(), "u".into());
    apply(
        &mut s,
        json!({"type":"tool_start","sessionId":"s","toolUseId":"tool1","toolName":"read","toolInput":{"path":"README.md"},"parentToolUseId":"parent"}),
    );
    apply(
        &mut s,
        json!({"type":"tool_result","sessionId":"s","toolUseId":"tool1","toolName":"read","result":"file contents"}),
    );
    assert_eq!(s.session.messages.len(), 2);
    let tool = &s.session.messages[1];
    assert_eq!(tool.tool_result.as_deref(), Some("file contents"));
    assert_eq!(tool.tool_status.as_deref(), Some("completed"));
    assert_eq!(tool.parent_tool_use_id.as_deref(), Some("parent"));
}
#[test]
fn foreign_or_late_events_cannot_restart_a_completed_session() {
    let mut s = state();
    s.begin_send("hi".into(), "u".into());
    apply(
        &mut s,
        json!({"type":"text_delta","sessionId":"other","delta":"wrong"}),
    );
    assert_eq!(s.session.messages.len(), 1);
    apply(&mut s, json!({"type":"interrupted","sessionId":"s"}));
    apply(
        &mut s,
        json!({"type":"text_delta","sessionId":"s","delta":"late"}),
    );
    assert_eq!(s.status, RunState::Interrupted);
    assert_eq!(s.session.messages.len(), 1);
}
#[test]
fn permission_and_send_failure_have_explicit_recovery() {
    let mut s = state();
    s.begin_send("test".into(), "u".into());
    apply(
        &mut s,
        json!({"type":"permission_request","sessionId":"s","request":{"sessionId":"s","requestId":"r","toolName":"bash","description":"run","canAlwaysAllow":true}}),
    );
    assert_eq!(s.status, RunState::WaitingForPermission);
    assert_eq!(s.permission.as_ref().unwrap().request_id, "r");
    s.permission_responded("r");
    assert_eq!(s.status, RunState::Processing);
    s.fail_send("u", "offline".into());
    assert_eq!(s.status, RunState::Idle);
    assert!(s.session.messages.is_empty());
    assert_eq!(s.last_error.as_deref(), Some("offline"));
}
#[test]
fn discard_removes_only_unfinished_text_for_that_turn() {
    let mut s = state();
    s.begin_send("hi".into(), "u".into());
    apply(
        &mut s,
        json!({"type":"text_delta","sessionId":"s","delta":"bad","turnId":"t"}),
    );
    apply(
        &mut s,
        json!({"type":"text_discard","sessionId":"s","turnId":"t"}),
    );
    assert_eq!(s.session.messages.len(), 1);
    apply(
        &mut s,
        json!({"type":"new_event","sessionId":"s","extra":true}),
    );
    assert_eq!(s.session.messages.len(), 1);
}

#[test]
fn an_error_does_not_allow_another_send_before_the_terminal_event() {
    let mut state = ChatState::new(Session {
        id: "s".into(),
        ..Session::default()
    });
    assert!(state.begin_send("first".into(), "u".into()));
    state.apply(
        &serde_json::from_value(
            serde_json::json!({"type":"error","sessionId":"s","error":"failed"}),
        )
        .unwrap(),
    );
    assert!(!state.begin_send("duplicate".into(), "u2".into()));
    state.apply(
        &serde_json::from_value(serde_json::json!({"type":"complete","sessionId":"s"})).unwrap(),
    );
    assert!(state.begin_send("retry".into(), "u3".into()));
}
