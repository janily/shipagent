//! Supplemental source-driven behavior and formatting cases.
#[test]
fn case_0_status_and_compaction_retain_card_continuity() {
    super::suite(
        "__supplemental__",
        "status and compaction retain card continuity",
    );
}
#[test]
fn case_1_explicit_interruption_marks_running_rows_and_active_todos() {
    super::suite(
        "__supplemental__",
        "explicit interruption marks running rows and active todos",
    );
}
#[test]
fn case_2_completion_promotes_intermediate_text_but_preserves_user_and_plan_boundaries() {
    super::suite(
        "__supplemental__",
        "completion promotes intermediate text but preserves user and plan boundaries",
    );
}
#[test]
fn case_3_tool_status_priorities_and_error_wrapper_cleanup() {
    super::suite(
        "__supplemental__",
        "tool status priorities and error wrapper cleanup",
    );
}
#[test]
fn case_4_nesting_depth_survives_late_parents_and_caps_cycles() {
    super::suite(
        "__supplemental__",
        "nesting depth survives late parents and caps cycles",
    );
}
#[test]
fn case_5_continued_groups_derive_running_error_and_completed_status() {
    super::suite(
        "__supplemental__",
        "continued groups derive running error and completed status",
    );
}
#[test]
fn case_6_agent_ids_use_first_matching_regex_capture_and_numeric_task_usage() {
    super::suite(
        "__supplemental__",
        "agent IDs use first matching regex capture and numeric task usage",
    );
}
#[test]
fn case_7_task_outputs_ignore_invalid_or_empty_usage_payloads() {
    super::suite(
        "__supplemental__",
        "task outputs ignore invalid or empty usage payloads",
    );
}
#[test]
fn case_8_empty_tool_identities_and_non_task_ancestry_stay_standalone() {
    super::suite(
        "__supplemental__",
        "empty tool identities and non task ancestry stay standalone",
    );
}
#[test]
fn case_9_todo_extraction_chooses_latest_valid_input_and_stable_equal_timestamps() {
    super::suite(
        "__supplemental__",
        "todo extraction chooses latest valid input and stable equal timestamps",
    );
}
#[test]
fn case_10_summaries_intent_and_selectors_preserve_empty_and_active_states() {
    super::suite(
        "__supplemental__",
        "summaries intent and selectors preserve empty and active states",
    );
}
#[test]
fn case_11_duration_and_token_formatting_retain_js_rounding_and_invalid_guards() {
    super::suite(
        "__supplemental__",
        "duration and token formatting retain JS rounding and invalid guards",
    );
}
#[test]
fn case_12_total_counts_and_last_children_include_groups_and_undefined_parents() {
    super::suite(
        "__supplemental__",
        "total counts and last children include groups and undefined parents",
    );
}
#[test]
fn case_13_background_pending_and_failed_rows_preserve_phase_predicates() {
    super::suite(
        "__supplemental__",
        "background pending and failed rows preserve phase predicates",
    );
}
#[test]
fn case_14_standalone_failures_complete_an_open_card_and_interruptions_handle_absent_todos() {
    super::suite(
        "__supplemental__",
        "standalone failures complete an open card and interruptions handle absent todos",
    );
}

#[test]
fn standalone_plan_opens_and_completes_a_card_without_prior_activities() {
    super::suite(
        "__supplemental__",
        "standalone plan opens and completes a card without prior activities",
    );
}
