#![allow(clippy::unwrap_used)]
//! Real bundled backend + scripted local model + actual read tool.
use agentkit_client::api::WorkbenchApi;
use agentkit_protocol::prototype::SessionEvent;
use agentkit_sidecar::{SidecarConfig, SidecarHandle};
use agentkit_store::{ChatState, RunState};
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    time::Duration,
};
struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
#[ignore = "Requires built backend and Node; AGENTKIT_BACKEND_DIR"]
fn real_agent_streams_executes_read_and_persists_the_final_answer() {
    let backend = std::env::var_os("AGENTKIT_BACKEND_DIR").unwrap();
    let temp = tempfile::tempdir().unwrap();
    let fixture = temp.path().join("proof.txt");
    std::fs::write(&fixture, "SHIPAGENT_TOOL_FIXTURE: real read result\n").unwrap();
    let script =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/fake-llm/server.mjs");
    let mut model = Process(
        Command::new("node")
            .arg(script)
            .env("AGENTKIT_FAKE_FIXTURE", &fixture)
            .env("AGENTKIT_FAKE_CHUNK_MS", "40")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let stdout = model.0.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = BufReader::new(stdout).read_line(&mut line);
        let _ = tx.send(line);
    });
    let line = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let url = line
        .trim()
        .strip_prefix("FAKE_LLM_URL=")
        .unwrap()
        .to_owned();
    smol::block_on(async {
        let sidecar = SidecarHandle::start(SidecarConfig::new(
            backend.into(),
            temp.path().join("config"),
        ))
        .await
        .unwrap();
        let endpoint = sidecar.endpoint();
        let initial = WorkbenchApi::connect(endpoint.clone(), String::new());
        let workspaces = initial.workspaces().await.unwrap();
        initial
            .setup_endpoint(&url, "fixture-model", None)
            .await
            .unwrap();
        initial.disable_browser().await.unwrap();
        initial.close();
        let api = WorkbenchApi::connect(endpoint, workspaces[0].id.clone());
        let events = api.session_events();
        let session = api
            .create_session(&workspaces[0].id, temp.path().to_str())
            .await
            .unwrap();
        let generated_snapshot = api.session_generated(&session.id).await.unwrap();
        assert_eq!(generated_snapshot.id, session.id);
        let mut state = ChatState::new(session);
        let id = state.session.id.clone();
        state.begin_send(
            "Read proof.txt and report its marker".into(),
            "test-user".into(),
        );
        let ack = api
            .send(&id, "Read proof.txt and report its marker", "test-user")
            .await
            .unwrap();
        assert!(ack.accepted);
        let mut kinds = Vec::new();
        let outcome = smol::future::race(
            async {
                loop {
                    let args = events.recv().await.unwrap();
                    let generated_event: agentkit_protocol::DecodedSessionEvent =
                        serde_json::from_value(args[0].clone()).unwrap();
                    assert!(
                        generated_event.known().is_some(),
                        "Unexpected event tag in pinned backend"
                    );
                    let event: SessionEvent = serde_json::from_value(args[0].clone()).unwrap();
                    if event.session_id != id {
                        continue;
                    }
                    kinds.push(event.kind.clone());
                    if matches!(event.kind.as_str(), "error" | "typed_error") {
                        eprintln!("backend test error: {}", event.payload["error"]);
                    }
                    state.apply(&event);
                    if event.kind == "complete" {
                        break true;
                    }
                }
            },
            async {
                smol::Timer::after(Duration::from_secs(60)).await;
                false
            },
        )
        .await;
        assert!(outcome, "Timed out waiting for complete");
        assert!(
            kinds.iter().any(|k| k == "text_delta"),
            "No streamed text; events: {kinds:?}"
        );
        assert!(
            kinds.iter().any(|k| k == "tool_start"),
            "No actual tool start; events: {kinds:?}"
        );
        assert!(
            kinds.iter().any(|k| k == "tool_result"),
            "No actual tool result; events: {kinds:?}"
        );
        assert_eq!(
            state.status,
            RunState::Idle,
            "Agent did not finish successfully"
        );
        assert!(state.session.messages.iter().any(|m| {
            m.tool_name.as_deref() == Some("Read")
                && m.tool_result
                    .as_deref()
                    .is_some_and(|r| r.contains("SHIPAGENT_TOOL_FIXTURE"))
        }));
        let generated_persisted = api.session_generated(&id).await.unwrap();
        assert!(
            generated_persisted
                .messages
                .iter()
                .any(|message| message.content.contains("SHIPAGENT_TOOL_FIXTURE"))
        );
        let persisted = api.session(&id).await.unwrap();
        assert_eq!(
            persisted
                .messages
                .iter()
                .filter(|m| m.role == "user")
                .count(),
            1
        );
        assert!(
            persisted
                .messages
                .iter()
                .any(|m| m.role == "assistant" && m.content.contains("SHIPAGENT_TOOL_FIXTURE"))
        );
        // A second round is cancelled after the first real streamed delta.
        let cancelled = api
            .create_session(&workspaces[0].id, temp.path().to_str())
            .await
            .unwrap();
        let cancelled_id = cancelled.id.clone();
        let mut cancelled_state = ChatState::new(cancelled);
        cancelled_state.begin_send("Read proof.txt again".into(), "cancel-user".into());
        assert!(
            api.send(&cancelled_id, "Read proof.txt again", "cancel-user")
                .await
                .unwrap()
                .accepted
        );
        let did_cancel = smol::future::race(
            async {
                let mut requested = false;
                loop {
                    let args = events.recv().await.unwrap();
                    let generated_event: agentkit_protocol::DecodedSessionEvent =
                        serde_json::from_value(args[0].clone()).unwrap();
                    assert!(generated_event.known().is_some());
                    let event: SessionEvent = serde_json::from_value(args[0].clone()).unwrap();
                    if event.session_id != cancelled_id {
                        continue;
                    }
                    cancelled_state.apply(&event);
                    if event.kind == "text_delta" && !requested {
                        api.cancel(&cancelled_id).await.unwrap();
                        requested = true;
                    }
                    if event.kind == "interrupted" {
                        break requested;
                    }
                    if event.kind == "complete" {
                        break false;
                    }
                }
            },
            async {
                smol::Timer::after(Duration::from_secs(20)).await;
                false
            },
        )
        .await;
        assert!(
            did_cancel,
            "No interrupted event after cancelling the real stream"
        );
        assert_eq!(cancelled_state.status, RunState::Interrupted);
        assert!(!api.session(&cancelled_id).await.unwrap().is_processing);
        api.close();
        sidecar.shutdown(Duration::from_secs(5)).await.unwrap();
    });
}
