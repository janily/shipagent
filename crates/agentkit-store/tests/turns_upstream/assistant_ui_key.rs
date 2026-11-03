//! Named counterparts of the original upstream test cases.
#[test]
fn case_0_uses_response_message_id_when_available() {
    super::suite(
        "assistant-ui-key.test.ts",
        "uses response message id when available",
    );
}
#[test]
fn case_1_disambiguates_split_cards_with_same_turnid_timestamp_via_index_fallback() {
    super::suite(
        "assistant-ui-key.test.ts",
        "disambiguates split cards with same turnId/timestamp via index fallback",
    );
}
