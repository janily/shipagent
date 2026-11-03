//! Named counterparts of the original upstream test cases.
#[test]
fn case_0_returns_pending_when_no_activities_and_not_complete() {
    super::suite(
        "turn-phase.test.ts",
        "returns pending when no activities and not complete",
    );
}
#[test]
fn case_1_returns_pending_when_turn_just_started_no_activities_no_response() {
    super::suite(
        "turn-phase.test.ts",
        "returns pending when turn just started (no activities, no response)",
    );
}
#[test]
fn case_2_returns_tool_active_when_any_activity_is_running() {
    super::suite(
        "turn-phase.test.ts",
        "returns tool_active when any activity is running",
    );
}
#[test]
fn case_3_returns_tool_active_with_mix_of_running_and_completed() {
    super::suite(
        "turn-phase.test.ts",
        "returns tool_active with mix of running and completed",
    );
}
#[test]
fn case_4_returns_tool_active_even_with_pending_activities_if_one_is_running() {
    super::suite(
        "turn-phase.test.ts",
        "returns tool_active even with pending activities if one is running",
    );
}
#[test]
fn case_5_returns_tool_active_with_multiple_running_tools() {
    super::suite(
        "turn-phase.test.ts",
        "returns tool_active with multiple running tools",
    );
}
#[test]
fn case_6_returns_awaiting_when_all_tools_complete_but_turn_not_complete() {
    super::suite(
        "turn-phase.test.ts",
        "returns awaiting when all tools complete but turn not complete",
    );
}
#[test]
fn case_7_returns_awaiting_with_multiple_completed_tools() {
    super::suite(
        "turn-phase.test.ts",
        "returns awaiting with multiple completed tools",
    );
}
#[test]
fn case_8_returns_awaiting_with_error_activities_errors_are_not_running() {
    super::suite(
        "turn-phase.test.ts",
        "returns awaiting with error activities (errors are not running)",
    );
}
#[test]
fn case_9_returns_awaiting_with_mix_of_completed_and_error_activities() {
    super::suite(
        "turn-phase.test.ts",
        "returns awaiting with mix of completed and error activities",
    );
}
#[test]
fn case_10_returns_awaiting_when_response_exists_but_is_not_streaming_non_streaming_response_means_complete_but()
 {
    super::suite(
        "turn-phase.test.ts",
        "returns awaiting when response exists but is not streaming (non-streaming response means complete, but isComplete takes precedence)",
    );
}
#[test]
fn case_11_returns_streaming_when_response_is_streaming() {
    super::suite(
        "turn-phase.test.ts",
        "returns streaming when response is streaming",
    );
}
#[test]
fn case_12_returns_streaming_when_response_is_streaming_after_tools() {
    super::suite(
        "turn-phase.test.ts",
        "returns streaming when response is streaming after tools",
    );
}
#[test]
fn case_13_streaming_takes_precedence_over_awaiting_when_response_exists() {
    super::suite(
        "turn-phase.test.ts",
        "streaming takes precedence over awaiting when response exists",
    );
}
#[test]
fn case_14_returns_complete_when_iscomplete_is_true() {
    super::suite(
        "turn-phase.test.ts",
        "returns complete when isComplete is true",
    );
}
#[test]
fn case_15_complete_takes_precedence_over_streaming() {
    super::suite(
        "turn-phase.test.ts",
        "complete takes precedence over streaming",
    );
}
#[test]
fn case_16_complete_takes_precedence_over_tool_active() {
    super::suite(
        "turn-phase.test.ts",
        "complete takes precedence over tool_active",
    );
}
#[test]
fn case_17_complete_takes_precedence_over_awaiting() {
    super::suite(
        "turn-phase.test.ts",
        "complete takes precedence over awaiting",
    );
}
#[test]
fn case_18_checks_complete_first_then_streaming_then_tool_active_then_awaiting_then_pending() {
    super::suite(
        "turn-phase.test.ts",
        "checks complete first, then streaming, then tool_active, then awaiting, then pending",
    );
}
#[test]
fn case_19_intermediate_text_with_running_status_returns_awaiting_not_tool_active() {
    super::suite(
        "turn-phase.test.ts",
        "intermediate text with running status returns awaiting (not tool_active)",
    );
}
#[test]
fn case_20_only_pending_tool_activities_returns_awaiting() {
    super::suite(
        "turn-phase.test.ts",
        "only pending tool activities returns awaiting",
    );
}
#[test]
fn case_21_status_activity_with_running_status_returns_awaiting() {
    super::suite(
        "turn-phase.test.ts",
        "status activity with running status returns awaiting",
    );
}
#[test]
fn case_22_backgrounded_tool_returns_awaiting() {
    super::suite("turn-phase.test.ts", "backgrounded tool returns awaiting");
}
#[test]
fn case_23_tool_running_intermediate_running_returns_tool_active() {
    super::suite(
        "turn-phase.test.ts",
        "tool running + intermediate running returns tool_active",
    );
}
#[test]
fn case_24_completed_tool_running_intermediate_returns_awaiting() {
    super::suite(
        "turn-phase.test.ts",
        "completed tool + running intermediate returns awaiting",
    );
}
#[test]
fn case_25_multiple_non_tool_running_activities_returns_awaiting() {
    super::suite(
        "turn-phase.test.ts",
        "multiple non-tool running activities returns awaiting",
    );
}
#[test]
fn case_26_returns_true_for_pending_phase() {
    super::suite("turn-phase.test.ts", "returns true for pending phase");
}
#[test]
fn case_27_returns_true_for_awaiting_phase_the_gap() {
    super::suite(
        "turn-phase.test.ts",
        "returns true for awaiting phase (the gap)",
    );
}
#[test]
fn case_28_returns_false_for_tool_active_phase_tools_are_visible() {
    super::suite(
        "turn-phase.test.ts",
        "returns false for tool_active phase (tools are visible)",
    );
}
#[test]
fn case_29_returns_true_for_streaming_when_buffering() {
    super::suite(
        "turn-phase.test.ts",
        "returns true for streaming when buffering",
    );
}
#[test]
fn case_30_returns_false_for_streaming_when_not_buffering() {
    super::suite(
        "turn-phase.test.ts",
        "returns false for streaming when not buffering",
    );
}
#[test]
fn case_31_returns_false_for_complete_phase() {
    super::suite("turn-phase.test.ts", "returns false for complete phase");
}
