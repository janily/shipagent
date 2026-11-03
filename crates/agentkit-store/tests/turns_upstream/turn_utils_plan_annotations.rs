//! Named counterparts of the original upstream test cases.
#[test]
fn case_0_keeps_plan_message_id_and_annotations_on_plan_activity_payload() {
    super::suite(
        "turn-utils-plan-annotations.test.ts",
        "keeps plan message id and annotations on plan activity payload",
    );
}
