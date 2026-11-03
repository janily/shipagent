//! Named counterparts of the original upstream test cases.
#[test]
fn case_0_returns_empty_array_for_empty_input() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "returns empty array for empty input",
    );
}
#[test]
fn case_1_returns_flat_list_when_no_task_tools_present() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "returns flat list when no Task tools present",
    );
}
#[test]
fn case_2_groups_child_activities_under_task_parent() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "groups child activities under Task parent",
    );
}
#[test]
fn case_3_maintains_chronological_order_of_children_within_group() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "maintains chronological order of children within group",
    );
}
#[test]
fn case_4_groups_children_under_an_agent_parent_identically_to_a_task_parent() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "groups children under an Agent parent identically to a Task parent",
    );
}
#[test]
fn case_5_groups_each_task_with_its_own_children() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "groups each Task with its own children",
    );
}
#[test]
fn case_6_preserves_orphan_activities_alongside_task_groups_in_chronological_order() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "preserves orphan activities alongside Task groups in chronological order",
    );
}
#[test]
fn case_7_attaches_taskoutput_data_to_parent_task_group_via_agentid_chain() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "attaches TaskOutput data to parent Task group via agentId chain",
    );
}
#[test]
fn case_8_handles_task_without_taskoutput_data_gracefully() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "handles Task without TaskOutput data gracefully",
    );
}
#[test]
fn case_9_hides_taskoutput_activities_from_result() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "hides TaskOutput activities from result",
    );
}
#[test]
fn case_10_handles_task_with_no_children_empty_group() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "handles Task with no children (empty group)",
    );
}
#[test]
fn case_11_groups_rows_of_a_subagent_launched_in_an_earlier_turn_into_one_continued_group_not_interleaved()
 {
    super::suite(
        "turn-utils-grouping.test.ts",
        "groups rows of a subagent launched in an earlier turn into one continued group, not interleaved",
    );
}
#[test]
fn case_12_handles_malformed_taskoutput_json_gracefully() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "handles malformed TaskOutput JSON gracefully",
    );
}
#[test]
fn case_13_out_of_order_stored_messages_children_before_parent_still_group_correctly() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "out-of-order stored messages: children before parent still group correctly",
    );
}
#[test]
fn case_14_restored_session_with_nested_task_hierarchy_produces_correct_groups() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "restored session with nested Task hierarchy produces correct groups",
    );
}
#[test]
fn case_15_mixed_top_level_and_nested_tools_group_into_correct_turns() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "mixed top-level and nested tools group into correct turns",
    );
}
#[test]
fn case_16_returns_empty_set_for_empty_input() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "returns empty set for empty input",
    );
}
#[test]
fn case_17_returns_empty_set_when_no_activities_have_parents_all_depth_0() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "returns empty set when no activities have parents (all depth 0)",
    );
}
#[test]
fn case_18_identifies_last_child_for_single_parent() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "identifies last child for single parent",
    );
}
#[test]
fn case_19_identifies_last_child_for_multiple_parents() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "identifies last child for multiple parents",
    );
}
#[test]
fn case_20_handles_nested_parent_child_relationships_depth_1() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "handles nested parent-child relationships (depth > 1)",
    );
}
#[test]
fn case_21_handles_single_child_is_both_first_and_last() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "handles single child (is both first and last)",
    );
}
#[test]
fn case_22_returns_true_for_activitygroup_objects() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "returns true for ActivityGroup objects",
    );
}
#[test]
fn case_23_returns_false_for_activityitem_objects() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "returns false for ActivityItem objects",
    );
}
#[test]
fn case_24_returns_false_for_activity_with_type_property_that_is_not_group() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "returns false for activity with type property that is not \"group\"",
    );
}
#[test]
fn case_25_includes_todowrite_activities_in_flat_list_not_grouped() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "includes TodoWrite activities in flat list (not grouped)",
    );
}
#[test]
fn case_26_handles_todowrite_as_child_of_task() {
    super::suite(
        "turn-utils-grouping.test.ts",
        "handles TodoWrite as child of Task",
    );
}
