//! Ported from mkagent v0.2.6 conversation-flow.test.ts
#[test]
fn renders_a_tool_call_its_result_and_the_final_assistant_response_in_order() {
    super::suite(
        "conversation-flow.test.ts",
        "renders a tool call, its result, and the final assistant response in order",
    );
}
