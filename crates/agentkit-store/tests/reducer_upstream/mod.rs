#![allow(clippy::unwrap_used)]
use agentkit_store::core::reducer::{Context, SessionState, process};
use serde_json::Value;
fn suite(source: &str, title: &str) {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/upstream-reducer.json")).unwrap();
    let mut count = 0;
    for record in fixture["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["source"] == source && r["title"] == title)
    {
        let state: SessionState = serde_json::from_value(record["input"].clone()).unwrap();
        let event = serde_json::from_value(record["event"].clone()).unwrap();
        let context: Context = serde_json::from_value(record["context"].clone()).unwrap();
        audit(
            &record["input"],
            &serde_json::to_value(&state).unwrap(),
            "input",
        );
        audit(
            &record["event"],
            &serde_json::to_value(&event).unwrap(),
            "event",
        );
        let before = serde_json::to_value(&state).unwrap();
        let actual = process(&state, &event, &context).unwrap();
        assert_eq!(serde_json::to_value(&state).unwrap(), before);
        assert_eq!(
            serde_json::to_value(process(&state, &event, &context).unwrap()).unwrap(),
            serde_json::to_value(&actual).unwrap()
        );
        let expected: agentkit_store::core::reducer::ProcessResult =
            serde_json::from_value(record["expected"].clone()).unwrap();
        audit(
            &record["expected"],
            &serde_json::to_value(&expected).unwrap(),
            "expected",
        );
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(expected).unwrap(),
            "{} / {}",
            source,
            record["title"]
        );
        count += 1;
    }
    assert!(count > 0);
}
mod conversation_flow;
mod handle_interrupted;
mod handle_user_message;
mod message_annotations_updated;
mod retry_lifecycle;
mod session_context_usage;
mod text_message_id_sync;
mod tool_result_input;

fn audit(source: &Value, typed: &Value, path: &str) {
    match (source, typed) {
        (Value::Object(source), Value::Object(typed)) => {
            for (key, value) in source {
                if let Some(next) = typed.get(key) {
                    audit(value, next, &format!("{path}.{key}"));
                } else {
                    assert!(
                        key == "motivation" && path.contains(".annotations["),
                        "unreviewed field lost from upstream fixture: {path}.{key}"
                    );
                }
            }
        }
        (Value::Array(source), Value::Array(typed)) => {
            assert_eq!(source.len(), typed.len(), "{path}");
            for (i, (a, b)) in source.iter().zip(typed).enumerate() {
                audit(a, b, &format!("{path}[{i}]"));
            }
        }
        (Value::Number(a), Value::Number(b)) => assert_eq!(a.as_f64(), b.as_f64(), "{path}"),
        _ => assert_eq!(source, typed, "{path}"),
    }
}

mod supplemental;
