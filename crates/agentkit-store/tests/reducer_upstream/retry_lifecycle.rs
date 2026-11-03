//! Ported from mkagent v0.2.6 retry-lifecycle.test.ts
#[test]
fn routes_text_discard_through_the_processor_and_removes_only_unfinished_assistant_text_for_its_turn()
 {
    super::suite(
        "retry-lifecycle.test.ts",
        "routes text_discard through the processor and removes only unfinished assistant text for its turn",
    );
}
#[test]
fn does_not_clear_streaming_state_belonging_to_a_different_turn() {
    super::suite(
        "retry-lifecycle.test.ts",
        "does not clear streaming state belonging to a different turn",
    );
}
#[test]
fn keeps_failed_attempts_out_of_a_later_successful_response_while_preserving_completed_intermediate_and()
 {
    super::suite(
        "retry-lifecycle.test.ts",
        "keeps failed attempts out of a later successful response while preserving completed, intermediate, and other-turn messages",
    );
}
#[test]
fn keeps_exhausted_partial_output_out_of_the_transcript_and_preserves_unrelated_messages() {
    super::suite(
        "retry-lifecycle.test.ts",
        "keeps exhausted partial output out of the transcript and preserves unrelated messages",
    );
}
#[test]
fn upserts_a_single_transient_retry_row_as_backoff_attempts_advance() {
    super::suite(
        "retry-lifecycle.test.ts",
        "upserts a single transient retry row as backoff attempts advance",
    );
}
#[test]
fn removes_the_transient_retry_row_and_indicator_on_retry_active() {
    super::suite(
        "retry-lifecycle.test.ts",
        "removes the transient retry row and indicator on retry active",
    );
}
#[test]
fn removes_the_transient_retry_row_and_indicator_on_retry_end() {
    super::suite(
        "retry-lifecycle.test.ts",
        "removes the transient retry row and indicator on retry end",
    );
}
#[test]
fn does_not_infer_retry_activation_from_a_text_delta_or_tool_start_during_backoff() {
    super::suite(
        "retry-lifecycle.test.ts",
        "does not infer retry activation from a text delta or tool start during backoff",
    );
}
#[test]
fn removes_retry_ui_as_a_fail_safe_on_complete() {
    super::suite(
        "retry-lifecycle.test.ts",
        "removes retry UI as a fail-safe on complete",
    );
}
#[test]
fn removes_retry_ui_as_a_fail_safe_on_plain_error() {
    super::suite(
        "retry-lifecycle.test.ts",
        "removes retry UI as a fail-safe on plain error",
    );
}
#[test]
fn removes_retry_ui_as_a_fail_safe_on_typed_error() {
    super::suite(
        "retry-lifecycle.test.ts",
        "removes retry UI as a fail-safe on typed error",
    );
}
#[test]
fn leaves_no_retry_status_spinner_after_interruption() {
    super::suite(
        "retry-lifecycle.test.ts",
        "leaves no retry status spinner after interruption",
    );
}
