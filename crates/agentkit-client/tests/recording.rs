#![allow(clippy::unwrap_used)]
use agentkit_client::recording::RecordingInput::{Events, Redact};
use serde_json::json;
#[test]
fn records_ordered_events_redacts_secrets_and_never_clobbers() {
    smol::block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let (tx, rx) = async_channel::bounded(8);
        tx.send(Redact("secret\"token".into())).await.unwrap();
        tx.send(Events(vec![
            json!({"type":"text_delta","delta":"abc secret\"token 中文"}),
        ]))
        .await
        .unwrap();
        tx.send(Events(vec![json!({"type":"complete"})]))
            .await
            .unwrap();
        drop(tx);
        agentkit_client::recording::record(rx, path.clone())
            .await
            .unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("secret"));
        let events: Vec<_> = text
            .lines()
            .map(|s| serde_json::from_str::<serde_json::Value>(s).unwrap())
            .collect();
        assert_eq!(events[0]["delta"], "abc <REDACTED> 中文");
        assert_eq!(events[1]["type"], "complete");
        let (_, rx) = async_channel::bounded(1);
        assert!(
            agentkit_client::recording::record(rx, path.clone())
                .await
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), text);
    });
}
#[test]
fn one_writer_survives_replaced_sources_and_redacts_rotated_tokens() {
    smol::block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("generations.jsonl");
        let (tx, rx) = async_channel::bounded(8);
        let worker = smol::spawn(agentkit_client::recording::record(rx, path.clone()));
        let old = tx.clone();
        old.send(Redact("old-token".into())).await.unwrap();
        old.send(Events(vec![
            json!({"type":"text_delta","delta":"old-token"}),
        ]))
        .await
        .unwrap();
        drop(old);
        let new = tx.clone();
        new.send(Redact("new-token".into())).await.unwrap();
        new.send(Events(vec![
            json!({"type":"text_delta","delta":"old-token new-token"}),
        ]))
        .await
        .unwrap();
        drop(new);
        drop(tx);
        worker.await.unwrap();
        let text = std::fs::read_to_string(path).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(!text.contains("old-token"));
        assert!(!text.contains("new-token"));
        assert_eq!(text.matches("<REDACTED>").count(), 3);
    });
}
