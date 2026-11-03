//! Ported from mkagent v0.2.6 text-message-id-sync.test.ts
#[test]
fn overwrites_existing_streaming_message_id_with_authoritative_messageid() {
    super::suite(
        "text-message-id-sync.test.ts",
        "overwrites existing streaming message id with authoritative messageId",
    );
}
#[test]
fn uses_authoritative_messageid_when_creating_message_in_race_path() {
    super::suite(
        "text-message-id-sync.test.ts",
        "uses authoritative messageId when creating message in race path",
    );
}
#[test]
fn keeps_backward_compatibility_when_messageid_is_missing() {
    super::suite(
        "text-message-id-sync.test.ts",
        "keeps backward compatibility when messageId is missing",
    );
}
