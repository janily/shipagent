//! Ported from mkagent v0.2.6 handle-user-message.test.ts
#[test]
fn applies_the_canonical_replay_timestamp_without_replacing_the_optimistic_id() {
    super::suite(
        "handle-user-message.test.ts",
        "applies the canonical replay timestamp without replacing the optimistic id",
    );
}
#[test]
fn keeps_the_completed_prior_answer_above_the_replayed_message_in_live_grouping() {
    super::suite(
        "handle-user-message.test.ts",
        "keeps the completed prior answer above the replayed message in live grouping",
    );
}
#[test]
fn ignores_a_late_queued_event_after_the_message_is_already_processing() {
    super::suite(
        "handle-user-message.test.ts",
        "ignores a late queued event after the message is already processing",
    );
}
