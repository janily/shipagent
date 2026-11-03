//! Supplemental source-driven branch scenarios.
#[test]
fn text_accumulation_fallback_and_intermediate_boundaries() {
    super::suite(
        "__supplemental__",
        "text accumulation fallback and intermediate boundaries",
    );
}
#[test]
fn fallback_to_existing_completed_assistant_text() {
    super::suite(
        "__supplemental__",
        "fallback to existing completed assistant text",
    );
}
#[test]
fn duplicate_authoritative_id_does_not_append_or_change_badge() {
    super::suite(
        "__supplemental__",
        "duplicate authoritative id does not append or change badge",
    );
}
#[test]
fn tool_inferred_errors_persisted_output_and_parent_orphan_completion() {
    super::suite(
        "__supplemental__",
        "tool inferred errors persisted output and parent orphan completion",
    );
}
#[test]
fn background_lifecycle_keeps_ownership_until_task_completed() {
    super::suite(
        "__supplemental__",
        "background lifecycle keeps ownership until task completed",
    );
}
#[test]
fn shell_backgrounding_and_absent_task_updates() {
    super::suite(
        "__supplemental__",
        "shell backgrounding and absent task updates",
    );
}
#[test]
fn status_compaction_and_information_transitions() {
    super::suite(
        "__supplemental__",
        "status compaction and information transitions",
    );
}
#[test]
fn metadata_and_deferred_effects_preserve_all_fields() {
    super::suite(
        "__supplemental__",
        "metadata and deferred effects preserve all fields",
    );
}
#[test]
fn queued_user_fallback_timestamp_window_and_accepted_transitions() {
    super::suite(
        "__supplemental__",
        "queued user fallback timestamp window and accepted transitions",
    );
}
#[test]
fn plain_failure_does_not_overwrite_completed_tools() {
    super::suite(
        "__supplemental__",
        "plain failure does not overwrite completed tools",
    );
}
#[test]
fn turnless_deltas_preserve_streaming_parent_and_fall_back_by_identity() {
    super::suite(
        "__supplemental__",
        "turnless deltas preserve streaming parent and fall back by identity",
    );
}
#[test]
fn typed_errors_keep_structured_retry_metadata_and_map_display_actions() {
    super::suite(
        "__supplemental__",
        "typed errors keep structured retry metadata and map display actions",
    );
}

#[test]
fn repeated_tool_starts_refresh_metadata_without_duplicating_identity() {
    super::suite(
        "__supplemental__",
        "repeated tool starts refresh metadata without duplicating identity",
    );
}

#[test]
fn completion_clears_queued_users_and_repairs_unfinished_tools() {
    super::suite(
        "__supplemental__",
        "completion clears queued users and repairs unfinished tools",
    );
}

#[test]
fn plan_payloads_retain_their_generated_message_role() {
    super::suite(
        "__supplemental__",
        "plan payloads retain their generated message role",
    );
}

#[test]
fn parent_completion_repairs_child_tools_with_missing_results() {
    super::suite(
        "__supplemental__",
        "parent completion repairs child tools with missing results",
    );
}

#[test]
fn interrupt_resets_pending_assistant_flags_without_discarding_partial_text() {
    super::suite(
        "__supplemental__",
        "interrupt resets pending assistant flags without discarding partial text",
    );
}

#[test]
fn plan_dispatch_preserves_a_user_payload_and_missing_annotations_target_is_ignored() {
    super::suite(
        "__supplemental__",
        "plan dispatch preserves a user payload and missing annotations target is ignored",
    );
}

#[test]
fn empty_turn_identity_must_not_overwrite_completed_history() {
    super::suite(
        "__supplemental__",
        "empty turn identity must not overwrite completed history",
    );
}

#[test]
fn tool_error_prefixes_use_ecmascript_whitespace_rules() {
    super::suite(
        "__supplemental__",
        "tool error prefixes use ECMAScript whitespace rules",
    );
}
