//! Named counterparts of the original upstream test cases.
#[test]
fn case_0_pending_streaming_complete_no_tools() {
    super::suite(
        "turn-lifecycle.test.ts",
        "pending → streaming → complete (no tools)",
    );
}
#[test]
fn case_1_pending_tool_active_awaiting_streaming_complete() {
    super::suite(
        "turn-lifecycle.test.ts",
        "pending → tool_active → awaiting → streaming → complete",
    );
}
#[test]
fn case_2_tool_active_awaiting_tool_active_awaiting_streaming_complete() {
    super::suite(
        "turn-lifecycle.test.ts",
        "tool_active → awaiting → tool_active → awaiting → streaming → complete",
    );
}
#[test]
fn case_3_handles_multiple_tools_running_in_parallel() {
    super::suite(
        "turn-lifecycle.test.ts",
        "handles multiple tools running in parallel",
    );
}
#[test]
fn case_4_error_transitions_to_awaiting_not_stuck_in_tool_active() {
    super::suite(
        "turn-lifecycle.test.ts",
        "error transitions to awaiting (not stuck in tool_active)",
    );
}
#[test]
fn case_5_user_message_during_tool_active_marks_turn_complete() {
    super::suite(
        "turn-lifecycle.test.ts",
        "user message during tool_active marks turn complete",
    );
}
#[test]
fn case_6_intermediate_text_during_tool_sequence_stays_in_awaiting() {
    super::suite(
        "turn-lifecycle.test.ts",
        "intermediate text during tool sequence stays in awaiting",
    );
}
#[test]
fn case_7_intermediate_text_completed_tool_session_done_phase_complete_intermediate_promoted_to_response()
 {
    super::suite(
        "turn-lifecycle.test.ts",
        "intermediate text + completed tool + session done → phase complete, intermediate promoted to response",
    );
}
#[test]
fn case_8_intermediate_text_completed_tool_session_still_processing_phase_awaiting() {
    super::suite(
        "turn-lifecycle.test.ts",
        "intermediate text + completed tool + session still processing → phase awaiting",
    );
}
#[test]
fn case_9_only_tool_session_done_no_text_phase_complete_no_response_promoted() {
    super::suite(
        "turn-lifecycle.test.ts",
        "only tool + session done + no text → phase complete, no response promoted",
    );
}
#[test]
fn case_10_omitting_the_option_keeps_current_behavior_backwards_compat() {
    super::suite(
        "turn-lifecycle.test.ts",
        "omitting the option keeps current behavior (backwards compat)",
    );
}
#[test]
fn case_11_empty_activities_array_returns_pending() {
    super::suite(
        "turn-lifecycle.test.ts",
        "empty activities array returns pending",
    );
}
#[test]
fn case_12_iscomplete_true_with_empty_activities_returns_complete() {
    super::suite(
        "turn-lifecycle.test.ts",
        "isComplete true with empty activities returns complete",
    );
}
#[test]
fn case_13_response_with_isstreaming_false_but_iscomplete_false_returns_awaiting() {
    super::suite(
        "turn-lifecycle.test.ts",
        "response with isStreaming false but isComplete false returns awaiting",
    );
}
#[test]
fn case_14_never_render_as_a_turn_but_the_assistant_reply_they_trigger_still_does() {
    super::suite(
        "turn-lifecycle.test.ts",
        "never render as a turn but the assistant reply they trigger still does",
    );
}
#[test]
fn case_15_a_visible_user_message_still_renders_normally_alongside_a_hidden_one() {
    super::suite(
        "turn-lifecycle.test.ts",
        "a visible user message still renders normally alongside a hidden one",
    );
}
