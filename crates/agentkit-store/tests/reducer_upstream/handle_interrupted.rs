//! Ported from mkagent v0.2.6 handle-interrupted.test.ts
#[test]
fn removes_queued_bubbles_and_emits_restore_input() {
    super::suite(
        "handle-interrupted.test.ts",
        "removes queued bubbles AND emits restore_input",
    );
}
#[test]
fn still_works_when_no_queued_bubbles_exist() {
    super::suite(
        "handle-interrupted.test.ts",
        "still works when no queued bubbles exist",
    );
}
#[test]
fn keeps_queued_bubbles_in_chat_and_does_not_emit_restore_input_616_fix() {
    super::suite(
        "handle-interrupted.test.ts",
        "KEEPS queued bubbles in chat and does NOT emit restore_input (#616 fix)",
    );
}
#[test]
fn marks_running_tools_as_interrupted_regardless_of_redirect_type() {
    super::suite(
        "handle-interrupted.test.ts",
        "marks running tools as interrupted regardless of redirect type",
    );
}
#[test]
fn always_strips_transient_status_messages() {
    super::suite(
        "handle-interrupted.test.ts",
        "always strips transient status messages",
    );
}
