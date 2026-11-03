//! Ported from mkagent v0.2.6 session-context-usage.test.ts
#[test]
fn preserves_post_compaction_snapshot_when_a_legacy_terminal_payload_omits_it() {
    super::suite(
        "session-context-usage.test.ts",
        "preserves post-compaction snapshot when a legacy terminal payload omits it",
    );
}
#[test]
fn preserves_occupancy_and_model_window_on_legacy_streaming_updates() {
    super::suite(
        "session-context-usage.test.ts",
        "preserves occupancy and model window on legacy streaming updates",
    );
}
#[test]
fn applies_419k_12k_immediately_and_preserves_explicit_unknown_state() {
    super::suite(
        "session-context-usage.test.ts",
        "applies 419k → 12k immediately and preserves explicit unknown state",
    );
}
