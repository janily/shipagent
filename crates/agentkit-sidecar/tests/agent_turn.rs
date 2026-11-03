#![allow(clippy::unwrap_used)]
//! Real bundled backend + scripted local model + actual read tool.
use agentkit_client::{ClientOptions, RpcClient, api::WorkbenchApi};
use agentkit_protocol::prototype::SessionEvent;
use agentkit_sidecar::{SidecarConfig, SidecarHandle};
use agentkit_store::core::reducer::{Context as ReductionContext, SessionState, process_decoded};
use agentkit_store::core::session::SessionModel;
use agentkit_store::core::turns::{
    GroupTurnsOptions, TurnPhase, derive_turn_phase, get_last_assistant_turn,
    group_messages_by_turn,
};
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
        let typed = RpcClient::connect(ClientOptions::local(
            endpoint.url.clone(),
            endpoint.token.clone(),
            workspaces[0].id.clone(),
        ));
        assert!(!typed.service().versions().await.unwrap().node.is_empty());
        typed.service().health().await.unwrap();
        assert_eq!(
            typed.workspaces().list().await.unwrap()[0].id,
            workspaces[0].id
        );
        typed
            .workspaces()
            .settings(&workspaces[0].id)
            .await
            .unwrap()
            .unwrap();
        typed
            .workspaces()
            .permissions(&workspaces[0].id)
            .await
            .unwrap();
        assert!(
            !typed
                .connections()
                .list_with_status()
                .await
                .unwrap()
                .is_empty()
        );
        typed.settings().default_thinking().await.unwrap();
        typed.settings().auto_compact_window().await.unwrap();
        typed.settings().send_message_key().await.unwrap();
        typed.settings().default_permissions().await.unwrap();
        typed.settings().network_proxy().await.unwrap();
        let api = WorkbenchApi::connect(endpoint, workspaces[0].id.clone());
        api.health().await.unwrap();
        let events = api.session_events();
        let session = api
            .create_session(&workspaces[0].id, temp.path().to_str())
            .await
            .unwrap();
        let draft = agentkit_protocol::generated::SessionDraft {
            text: "typed API persisted draft".into(),
            attachments: None,
        };
        typed.drafts().set(&session.id, &draft).await.unwrap();
        assert_eq!(
            typed.drafts().get(&session.id).await.unwrap().unwrap().text,
            draft.text
        );
        typed.drafts().delete(&session.id).await.unwrap();
        assert!(typed.drafts().get(&session.id).await.unwrap().is_none());
        assert!(
            typed
                .sessions()
                .list()
                .await
                .unwrap()
                .iter()
                .any(|s| s.id == session.id)
        );
        // With no pending plan the real backend declines the dispatch claim normally.
        assert!(
            matches!(typed.sessions().command(&session.id,
            &agentkit_protocol::generated::SessionCommand::MarkPendingPlanExecutionDispatched
        ).await.unwrap(), agentkit_client::api::types::CommandResult::PlanDispatched(false))
        );
        assert!(!api.workspaces_generated().await.unwrap().is_empty());
        api.workspace_settings(&workspaces[0].id).await.unwrap();
        api.workspace_skills(&workspaces[0].id).await.unwrap();
        api.connections_generated().await.unwrap();
        api.unread_snapshot().await.unwrap();
        api.view_session(&session.id, &workspaces[0].id)
            .await
            .unwrap();
        let generated_snapshot = api.session_generated(&session.id).await.unwrap();
        assert_eq!(generated_snapshot.id, session.id);
        let mut cache = SessionModel::new(generated_snapshot.clone());
        let mut formal = SessionState {
            session: generated_snapshot,
            streaming: None,
        };
        let mut reduction_index = 0;
        let mut live_phases = Vec::new();
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
        let mut observed_tool_result = None;
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
                    if event.kind == "tool_result" {
                        observed_tool_result = Some((
                            event.payload["toolUseId"].clone(),
                            event.payload["result"].clone(),
                        ));
                    }
                    reduction_index += 1;
                    let result = process_decoded(
                        &formal,
                        &generated_event,
                        &ReductionContext {
                            now_ms: 1700000000000 + reduction_index,
                            message_id: format!("formal-live-{reduction_index}"),
                        },
                    )
                    .unwrap();
                    formal = result.state;
                    cache
                        .apply(
                            &generated_event,
                            &ReductionContext {
                                now_ms: 1700000000000 + reduction_index,
                                message_id: format!("formal-live-{reduction_index}"),
                            },
                        )
                        .unwrap();
                    assert_eq!(
                        serde_json::to_value(&cache.turns).unwrap(),
                        serde_json::to_value(group_messages_by_turn(
                            &cache.state.session.messages,
                            GroupTurnsOptions {
                                is_session_processing: Some(cache.state.session.is_processing)
                            }
                        ))
                        .unwrap()
                    );
                    let turns = group_messages_by_turn(
                        &formal.session.messages,
                        GroupTurnsOptions {
                            is_session_processing: Some(formal.session.is_processing),
                        },
                    );
                    if let Some(turn) = get_last_assistant_turn(&turns) {
                        live_phases.push(derive_turn_phase(turn));
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
        // The native user-row branch action must work against the pinned backend,
        // not just construct a matching wire request.
        let original_user = generated_persisted
            .messages
            .iter()
            .find(|m| m.role == agentkit_protocol::generated::MessageRole::User)
            .unwrap();
        let branch = api
            .branch_session_generated(&workspaces[0].id, &id, &original_user.id, None)
            .await
            .unwrap();
        assert_ne!(branch.id, id);
        assert_eq!(branch.workspace_id, workspaces[0].id);
        assert!(branch.messages.iter().any(|m| m.id == original_user.id));
        assert!(!formal.session.is_processing);
        assert!(formal.streaming.is_none());
        let formal_answer = formal
            .session
            .messages
            .iter()
            .find(|m| {
                m.role == agentkit_protocol::generated::MessageRole::Assistant
                    && m.content.contains("SHIPAGENT_TOOL_FIXTURE")
            })
            .unwrap();
        assert!(
            generated_persisted
                .messages
                .iter()
                .any(|m| m.id == formal_answer.id && m.content == formal_answer.content),
            "Formal reducer did not retain authoritative final message identity/content"
        );
        let grouped = group_messages_by_turn(
            &formal.session.messages,
            GroupTurnsOptions {
                is_session_processing: Some(false),
            },
        );
        let final_turn = get_last_assistant_turn(&grouped).unwrap();
        assert_eq!(derive_turn_phase(final_turn), TurnPhase::Complete);
        assert_eq!(
            final_turn.response.as_ref().unwrap().message_id.as_deref(),
            Some(formal_answer.id.as_str())
        );
        assert_eq!(
            final_turn.response.as_ref().unwrap().text,
            formal_answer.content
        );
        assert!(
            final_turn
                .activities
                .iter()
                .any(|a| a.tool_name.as_deref() == Some("Read"))
        );
        assert!(
            live_phases.contains(&TurnPhase::ToolActive),
            "Live grouping never represented the actual running tool"
        );
        assert!(!cache.busy());
        assert!(
            cache
                .state
                .session
                .messages
                .iter()
                .any(|m| m.id == formal_answer.id && m.content == formal_answer.content)
        );
        let formal_tool = formal
            .session
            .messages
            .iter()
            .find(|m| {
                m.tool_result
                    .as_ref()
                    .is_some_and(|r| r.contains("SHIPAGENT_TOOL_FIXTURE"))
            })
            .unwrap();
        assert_eq!(
            formal_tool.tool_status,
            Some(agentkit_protocol::generated::ToolStatus::Completed)
        );
        // Compact backend transcripts intentionally omit tool rows (SessionManager
        // tool_result). Validate live tool identity/content against the actual wire
        // event, and final assistant identity/content against durable history above.
        let (tool_id, tool_result) = observed_tool_result.unwrap();
        assert_eq!(tool_id.as_str(), formal_tool.tool_use_id.as_deref());
        assert_eq!(tool_result.as_str(), formal_tool.tool_result.as_deref());
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
        let mut formal_cancelled = SessionState {
            session: api.session_generated(&cancelled_id).await.unwrap(),
            streaming: None,
        };
        let mut cancelled_cache = SessionModel::new(formal_cancelled.session.clone());
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
                    reduction_index += 1;
                    formal_cancelled = process_decoded(
                        &formal_cancelled,
                        &generated_event,
                        &ReductionContext {
                            now_ms: 1700000000000 + reduction_index,
                            message_id: format!("formal-live-{reduction_index}"),
                        },
                    )
                    .unwrap()
                    .state;
                    cancelled_cache
                        .apply(
                            &generated_event,
                            &ReductionContext {
                                now_ms: 1700000000000 + reduction_index,
                                message_id: format!("formal-live-{reduction_index}"),
                            },
                        )
                        .unwrap();
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
        assert!(!cancelled_cache.busy());
        assert!(cancelled_cache.state.streaming.is_none());
        assert!(!formal_cancelled.session.is_processing);
        assert!(formal_cancelled.streaming.is_none());
        let cancelled_turns = group_messages_by_turn(
            &formal_cancelled.session.messages,
            GroupTurnsOptions {
                is_session_processing: Some(false),
            },
        );
        assert!(
            get_last_assistant_turn(&cancelled_turns)
                .is_some_and(|t| derive_turn_phase(t) == TurnPhase::Complete)
        );
        assert!(!api.session(&cancelled_id).await.unwrap().is_processing);
        typed.close();
        api.close();
        sidecar.shutdown(Duration::from_secs(5)).await.unwrap();
    });
}
