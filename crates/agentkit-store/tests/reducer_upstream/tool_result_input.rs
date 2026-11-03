//! Ported from mkagent v0.2.6 tool-result-input.test.ts
#[test]
fn replaces_the_input_from_tool_start_with_the_one_sent_on_the_result() {
    super::suite(
        "tool-result-input.test.ts",
        "replaces the input from tool_start with the one sent on the result",
    );
}
#[test]
fn keeps_the_input_from_tool_start_when_the_result_carries_none() {
    super::suite(
        "tool-result-input.test.ts",
        "keeps the input from tool_start when the result carries none",
    );
}
#[test]
fn stores_the_input_on_a_message_created_from_a_result_without_tool_start() {
    super::suite(
        "tool-result-input.test.ts",
        "stores the input on a message created from a result without tool_start",
    );
}
