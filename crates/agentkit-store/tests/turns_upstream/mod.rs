#![allow(clippy::unwrap_used)]
use agentkit_store::core::turns::*;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
fn typed<T: DeserializeOwned + Serialize>(v: &Value) -> T {
    let x: T = serde_json::from_value(v.clone()).unwrap();
    audit(v, &serde_json::to_value(&x).unwrap());
    x
}
fn audit(source: &Value, typed: &Value) {
    match (source, typed) {
        (Value::Object(a), Value::Object(b)) => {
            for (key, value) in a {
                audit(
                    value,
                    b.get(key)
                        .unwrap_or_else(|| panic!("unreviewed field lost: {key}")),
                );
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                audit(a, b);
            }
        }
        (Value::Number(a), Value::Number(b)) => assert_eq!(a.as_f64(), b.as_f64()),
        _ => assert_eq!(source, typed),
    }
}
fn value(x: impl Serialize) -> Value {
    serde_json::to_value(x).unwrap()
}
fn number(v: &Value) -> f64 {
    if let Some(s) = v.get("$number").and_then(Value::as_str) {
        match s {
            "NaN" => f64::NAN,
            "-0" => -0.0,
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            _ => panic!("unknown special number"),
        }
    } else {
        v.as_f64().unwrap()
    }
}
fn call(handler: &str, args: &[Value]) -> Value {
    match handler {
        "groupMessagesByTurn" => value(group_messages_by_turn(
            &typed::<Vec<agentkit_protocol::generated::Message>>(&args[0]),
            args.get(1)
                .filter(|v| !v.is_null())
                .map(typed)
                .unwrap_or_default(),
        )),
        "deriveTurnPhase" => value(derive_turn_phase(&typed(&args[0]))),
        "shouldShowThinkingIndicator" => value(should_show_thinking_indicator(
            typed(&args[0]),
            args[1].as_bool().unwrap(),
        )),
        "getAssistantTurnUiKey" => value(get_assistant_turn_ui_key(
            &typed(&args[0]),
            args[1].as_u64().unwrap() as usize,
        )),
        "groupActivitiesByParent" => value(group_activities_by_parent(&typed::<Vec<Activity>>(
            &args[0],
        ))),
        "isActivityGroup" => value(is_activity_group(&typed(&args[0]))),
        "computeLastChildSet" => {
            let mut x: Vec<_> = compute_last_child_set(&typed::<Vec<Activity>>(&args[0]))
                .into_iter()
                .collect();
            x.sort();
            value(x)
        }
        "formatMessageTimestamp" => value(format_message_timestamp(args[0].as_i64().unwrap(), 0)),
        "getTurnIntent" => value(get_turn_intent(&typed(&args[0]))),
        "hasPendingActivities" => value(has_pending_activities(&typed(&args[0]))),
        "hasErrorActivities" => value(has_error_activities(&typed(&args[0]))),
        "getActivitySummary" => value(activity_summary(&typed(&args[0]))),
        "getLastAssistantTurn" => value(get_last_assistant_turn(&typed::<Vec<Turn>>(&args[0]))),
        "getLastUserMessageTime" => {
            value(get_last_user_message_time(&typed::<Vec<Turn>>(&args[0])))
        }
        "isLastTurnStreaming" => value(is_last_turn_streaming(&typed::<Vec<Turn>>(&args[0]))),
        "countTotalActivities" => value(count_total_activities(&typed::<Vec<ActivityOrGroup>>(
            &args[0],
        ))),
        "formatDuration" => value(format_duration(number(&args[0]))),
        "formatTokens" => value(format_tokens(number(&args[0]))),
        name => panic!("Unported oracle call {name}"),
    }
}
fn suite(source: &str, title: &str) {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/upstream-turns.json")).unwrap();
    let mut count = 0;
    for r in fixture["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["source"] == source && r["title"] == title)
    {
        let args = r["args"].as_array().unwrap();
        let handler = r["handler"].as_str().unwrap();
        let mut expected = r["expected"].clone();
        if handler == "computeLastChildSet" {
            expected
                .as_array_mut()
                .unwrap()
                .sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        }
        let before = args.clone();
        let actual = call(handler, args);
        audit(&expected, &actual);
        audit(&actual, &expected);
        assert_eq!(call(handler, args), actual);
        assert_eq!(args, &before);
        count += 1;
    }
    assert!(count > 0, "No captured calls for {source} / {title}");
}
#[test]
fn every_original_case_is_captured_or_explicitly_an_upstream_react_source_guard() {
    let x: Value = serde_json::from_str(include_str!("../fixtures/upstream-turns.json")).unwrap();
    let guards = x["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["source"] == "accept-plan-chevron-group.test.ts")
        .count();
    assert_eq!(guards, 2);
    for c in x["cases"].as_array().unwrap() {
        if c["source"] != "accept-plan-chevron-group.test.ts" {
            assert!(
                x["records"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["source"] == c["source"] && r["title"] == c["title"])
            );
        }
    }
    assert_eq!(
        x["sourceGuards"],
        json!(["accept-plan-chevron-group.test.ts"])
    );
}
mod assistant_ui_key;
mod message_timestamp;
mod turn_lifecycle;
mod turn_phase;
mod turn_utils_grouping;
mod turn_utils_plan_annotations;

mod supplemental;
