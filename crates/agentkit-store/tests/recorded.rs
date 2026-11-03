#![allow(clippy::unwrap_used)]
//! Offline recorded-wire replay; requires no backend or model.
use agentkit_store::fixtures::{SCENARIOS, replay, verify_scenario};
use serde_json::Value;
use sha2::{Digest, Sha256};
fn load(name: &str) -> (String, String, String, Value) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/recorded");
    let read = |suffix| std::fs::read_to_string(root.join(format!("{name}.{suffix}"))).unwrap();
    (
        read("initial.json"),
        read("jsonl"),
        read("final.json"),
        serde_json::from_str(&read("meta.json")).unwrap(),
    )
}
#[test]
fn all_required_real_backend_recordings_replay_and_have_integrity() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source: Value =
        serde_json::from_slice(&std::fs::read(root.join("tools/codegen/sources.json")).unwrap())
            .unwrap();
    let digest = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    for name in SCENARIOS {
        let (initial, events, final_state, meta) = load(name);
        replay(&initial, &events, &final_state).unwrap_or_else(|e| panic!("{name}: {e}"));
        let evidence = verify_scenario(name, &initial, &events, &final_state)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(meta["evidence"], evidence);
        assert_eq!(meta["schemaVersion"], 1);
        assert_eq!(meta["projectionVersion"], 1);
        assert_eq!(meta["scenario"], *name);
        assert_eq!(meta["upstreamCommit"], source["backendRevision"]);
        for (key, bytes) in [
            ("initial", initial.as_bytes()),
            ("events", events.as_bytes()),
            ("final", final_state.as_bytes()),
        ] {
            assert_eq!(meta["files"][key], digest(bytes), "{name} {key}");
        }
        assert_eq!(
            meta["modelSha256"],
            digest(&std::fs::read(root.join("tools/fixtures/model.mjs")).unwrap())
        );
        assert_eq!(
            meta["scenarioSha256"],
            digest(
                &std::fs::read(root.join(format!("tools/fake-llm/scenarios/{name}.yaml"))).unwrap()
            )
        );
    }
}
#[test]
fn corrupt_missing_foreign_and_unknown_events_fail_replay() {
    let (initial, events, final_state, _) = load("text");
    assert!(replay(&initial, "", &final_state).is_err());
    assert!(replay(&initial, "not-json", &final_state).is_err());
    assert!(
        replay(
            &initial,
            "{\"type\":\"future\",\"sessionId\":\"missing\"}",
            &final_state
        )
        .is_err()
    );
    assert!(replay("{}", &events, &final_state).is_err());
    let mut rows: Vec<Value> = events
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    rows[0]["sessionId"] = "missing".into();
    let changed = rows
        .iter()
        .map(|e| serde_json::to_string(e).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(replay(&initial, &changed, &final_state).is_err());
    let changed = final_state.replace("Fixture answer", "Tampered answer");
    assert!(replay(&initial, &events, &changed).is_err());
}
#[test]
fn an_idle_run_cannot_impersonate_an_unexercised_scenario() {
    let (initial, events, final_state, _) = load("text");
    for name in SCENARIOS.iter().filter(|n| **n != "text") {
        assert!(
            verify_scenario(name, &initial, &events, &final_state).is_err(),
            "{name}"
        );
    }
    assert!(verify_scenario("bogus", &initial, &events, &final_state).is_err());
    let incomplete = events
        .lines()
        .filter(|l| !l.contains("\"type\":\"complete\""))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(verify_scenario("text", &initial, &incomplete, &final_state).is_err());
}
#[test]
fn durable_error_details_retryability_and_original_are_compared() {
    let (initial, events, final_state, _) = load("typed-error");
    for (field, value) in [
        ("errorCanRetry", serde_json::json!(true)),
        ("errorDetails", serde_json::json!("tampered details")),
        ("errorOriginal", serde_json::json!("tampered original")),
    ] {
        let mut state: Value = serde_json::from_str(&final_state).unwrap();
        for session in state.as_object_mut().unwrap().values_mut() {
            for message in session["messages"].as_array_mut().unwrap() {
                if message["role"] == "error" {
                    message[field] = value.clone();
                }
            }
        }
        assert!(
            replay(&initial, &events, &serde_json::to_string(&state).unwrap()).is_err(),
            "{field}"
        );
    }
}
