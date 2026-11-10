#![allow(clippy::unwrap_used)]
use super::*;
use crate::controller::oauth_test_support as support;
use serde_json::json;
use support::*;
#[test]
fn persisted_dispatch_and_acceptance_use_exact_claim_and_ack_order() {
    for mode in 0..12 {
        run(mode);
    }
}
#[cfg(test)]
fn run(mode: u8) {
    let (tx, rx) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            tx.send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            send(&mut ws,json!({"type":"handshake_ack","clientId":"fixture","registeredChannels":agentkit_protocol::constants::channels::ALL})).await;
            let mut claimed = false;
            let mut prepared = false;
            let mut switched = false;
            let mut sent = false;
            loop {
                let req = frame(&mut ws).await;
                let channel = req["channel"].as_str().unwrap();
                let result = match channel {
                    "sessions:getMessages" => {
                        json!({"id":"s","workspaceId":"w","workspaceName":"W","messages":[{"id":"p","role":"plan","content":"/plan.md","timestamp":1}],"lastMessageAt":1,"isProcessing":mode==4,"permissionMode":if (7..=10).contains(&mode){"safe"}else{"ask"}})
                    }
                    "sessions:getPendingPlanExecution" => {
                        if (7..=10).contains(&mode) {
                            serde_json::Value::Null
                        } else {
                            json!({"planPath":"/plan.md","draftInputSnapshot":"saved context","awaitingCompaction":mode==1,"executionDispatched":mode==3})
                        }
                    }
                    "sessions:command" => match req["args"][1]["type"].as_str().unwrap() {
                        "markPendingPlanExecutionDispatched" => {
                            assert!(
                                !(7..=10).contains(&mode) && mode != 1 && mode != 3 && mode != 4
                            );
                            claimed = true;
                            if mode == 6 {
                                serde_json::Value::Null
                            } else {
                                json!(mode != 2)
                            }
                        }
                        "setPermissionMode" => {
                            switched = true;
                            assert!((7..=10).contains(&mode));
                            assert_eq!(req["args"][1]["mode"], "allow-all");
                            serde_json::Value::Null
                        }
                        "setPendingPlanExecution" => {
                            assert!(switched && mode >= 8);
                            prepared = true;
                            assert_eq!(req["args"][1]["planPath"], "/plan.md");
                            assert_eq!(req["args"][1]["draftInputSnapshot"], "new context");
                            serde_json::Value::Null
                        }
                        "clearPendingPlanExecution" => {
                            assert!(claimed && sent && matches!(mode, 0 | 11));
                            serde_json::Value::Null
                        }
                        _ => panic!("unexpected command"),
                    },
                    "sessions:sendMessage" => {
                        sent = true;
                        if (7..=10).contains(&mode) {
                            assert!(switched);
                            if mode >= 8 {
                                assert!(prepared);
                                assert_eq!(req["args"][1], "/compact");
                            } else {
                                assert_eq!(
                                    req["args"][1],
                                    "approved\n\n---\n\n**additional**\n\nnew context"
                                );
                            }
                        } else {
                            assert!(claimed);
                            assert_eq!(
                                req["args"][1],
                                "approved\n\n---\n\n**additional**\n\nsaved context"
                            );
                        }
                        json!({"accepted":mode!=5,"messageId":"sent"})
                    }
                    _ => panic!("unexpected channel"),
                };
                if (mode == 9 && req["args"][1]["type"] == "setPendingPlanExecution")
                    || (mode == 11 && req["args"][1]["type"] == "clearPendingPlanExecution")
                {
                    send(&mut ws,json!({"id":req["id"],"type":"error","channel":channel,"error":{"code":"HANDLER_ERROR","message":"fixture failure"}})).await;
                    if mode == 11 {
                        no_more_requests(&mut ws).await;
                    }
                    break;
                }
                send(
                    &mut ws,
                    json!({"id":req["id"],"type":"response","channel":channel,"result":result}),
                )
                .await;
                let done = match mode {
                    0 => req["args"][1]["type"] == "clearPendingPlanExecution",
                    1 | 3 => channel == "sessions:getPendingPlanExecution",
                    2 | 6 => req["args"][1]["type"] == "markPendingPlanExecutionDispatched",
                    4 | 10 => channel == "sessions:getMessages",
                    5 | 7 | 8 => channel == "sessions:sendMessage",
                    _ => false,
                };
                if done {
                    if mode == 5 {
                        no_more_requests(&mut ws).await;
                    }
                    break;
                }
            }
        })
    });
    smol::block_on(async {
        let api = WorkbenchApi::connect(
            agentkit_client::Endpoint {
                url: rx.recv().unwrap(),
                token: "fixture".into(),
            },
            "w".into(),
        );
        let result = if (7..=10).contains(&mode) {
            accept(
                &api,
                "s",
                if mode == 10 { "old" } else { "p" },
                mode >= 8,
                "new context",
                "approved",
                "additional",
            )
            .await
        } else {
            if matches!(mode, 5 | 11) {
                let scheduler = policy::Scheduler::default();
                let flight = scheduler.request("s").unwrap();
                assert!(scheduler.request("s").is_none());
                let result = drain(&api, "s", "approved", "additional", flight).await;
                assert!(scheduler.request("s").is_some());
                result
            } else {
                dispatch(&api, "s", "approved", "additional").await
            }
        };
        assert_eq!(result.is_err(), matches!(mode, 5 | 6 | 9 | 10 | 11));
    });
    server.join().unwrap();
}

#[cfg(test)]
async fn no_more_requests(ws: &mut Socket) {
    use futures::StreamExt;
    let repeated = smol::future::race(
        async {
            matches!(
                ws.next().await,
                Some(Ok(async_tungstenite::tungstenite::Message::Text(_)))
            )
        },
        async {
            smol::Timer::after(std::time::Duration::from_millis(30)).await;
            false
        },
    )
    .await;
    assert!(!repeated, "uncertain plan dispatch was retried");
}

struct ModelProcess(std::process::Child);
impl Drop for ModelProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[test]
#[ignore = "Requires bundled backend and Node; isolated local model, no provider accounts"]
fn persisted_plan_compacts_and_resumes_after_backend_restart() {
    use agentkit_sidecar::{SidecarConfig, SidecarHandle};
    use std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
        time::Duration,
    };
    smol::block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        let backend = std::path::PathBuf::from(std::env::var_os("AGENTKIT_BACKEND_DIR").unwrap());
        let sidecar = SidecarHandle::start(SidecarConfig::new(backend.clone(), config.clone()))
            .await
            .unwrap();
        let initial = WorkbenchApi::connect(sidecar.endpoint(), String::new());
        let workspace = initial.workspaces_generated().await.unwrap().remove(0);
        let root = std::path::PathBuf::from(&workspace.root_path);
        std::fs::create_dir_all(&root).unwrap();
        let plan = root.join("plan.md");
        std::fs::write(&plan, "# Actual plan\n\n1. Inspect.\n2. Verify.\n").unwrap();
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut model = ModelProcess(
            Command::new("node")
                .arg(repo.join("tools/fixtures/model.mjs"))
                .arg(repo.join("tools/fake-llm/scenarios/plan.yaml"))
                .env("AGENTKIT_FIXTURE_ROOT", &root)
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
        let url = rx
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .trim()
            .strip_prefix("FAKE_LLM_URL=")
            .unwrap()
            .to_string();
        initial
            .setup_endpoint(&url, "fixture-model", None)
            .await
            .unwrap();
        initial.disable_browser().await.unwrap();
        initial.close();
        let api = WorkbenchApi::connect(sidecar.endpoint(), workspace.id.clone());
        let session = api
            .create_session(&workspace.id, root.to_str())
            .await
            .unwrap();
        let id = session.id;
        assert!(
            api.send(&id, "Prepare a plan.", "plan-fixture-start")
                .await
                .unwrap()
                .accepted
        );
        let plan_deadline = std::time::Instant::now() + Duration::from_secs(40);
        let ready = loop {
            let snapshot = api.session_generated(&id).await.unwrap();
            if !snapshot.is_processing
                && snapshot
                    .messages
                    .iter()
                    .any(|m| m.role == g::MessageRole::Plan)
            {
                break snapshot;
            }
            assert!(
                std::time::Instant::now() < plan_deadline,
                "SubmitPlan did not become observable: {:?}",
                snapshot.messages
            );
            smol::Timer::after(Duration::from_millis(30)).await;
        };
        let message = ready
            .messages
            .iter()
            .find(|m| m.role == g::MessageRole::Plan)
            .unwrap();
        let path = policy::path(message).unwrap();
        assert!(api.plan_text(path).await.unwrap().contains("Actual plan"));
        let events = api.session_events();
        let queued = accept(
            &api,
            &id,
            &message.id,
            true,
            "persisted user context",
            "approved",
            "additional",
        )
        .await
        .unwrap();
        assert!(matches!(queued, Outcome::Pending { .. }));
        // First prove failed manual compaction cannot authorize execution.
        let failed_deadline = std::time::Instant::now() + Duration::from_secs(15);
        let mut failed = false;
        loop {
            while let Ok(batch) = events.try_recv() {
                failed |= batch.iter().any(|e| {
                    e["type"] == "error"
                        && e["error"]
                            .as_str()
                            .is_some_and(|s| s.contains("Nothing to compact"))
                });
            }
            if failed && !api.session_generated(&id).await.unwrap().is_processing {
                break;
            }
            assert!(
                std::time::Instant::now() < failed_deadline,
                "small fixture did not report failed compaction"
            );
            smol::Timer::after(Duration::from_millis(20)).await;
        }
        assert!(
            api.pending_plan(&id)
                .await
                .unwrap()
                .unwrap()
                .awaiting_compaction
        );
        assert!(matches!(
            dispatch(&api, &id, "approved", "additional").await.unwrap(),
            Outcome::Pending {
                awaiting_compaction: true,
                ..
            }
        ));
        api.clear_plan(&id).await.unwrap();
        // Keep a complete recent turn larger than the pinned SDK's 20k-token
        // retention budget so there is older history to summarize legitimately.
        assert!(
            api.send(&id, &"history ".repeat(16000), "plan-fixture-history")
                .await
                .unwrap()
                .accepted
        );
        let _ = wait_idle(&api, &id).await;
        assert!(matches!(
            accept(
                &api,
                &id,
                &message.id,
                true,
                "persisted user context",
                "approved",
                "additional"
            )
            .await
            .unwrap(),
            Outcome::Pending { .. }
        ));
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let pending = loop {
            let p = api.pending_plan(&id).await.unwrap().unwrap();
            if !p.awaiting_compaction && !api.session_generated(&id).await.unwrap().is_processing {
                break p;
            }
            if std::time::Instant::now() > deadline {
                let events: Vec<_> = std::iter::from_fn(|| events.try_recv().ok())
                    .flatten()
                    .filter(|e| {
                        matches!(
                            e["type"].as_str(),
                            Some("info" | "error" | "typed_error" | "complete")
                        )
                    })
                    .collect();
                panic!("manual compaction never became ready: {:?}", events);
            }
            smol::Timer::after(Duration::from_millis(30)).await;
        };
        assert_ne!(pending.execution_dispatched, Some(true));
        assert_eq!(
            pending.draft_input_snapshot.as_deref(),
            Some("persisted user context")
        );
        api.close();
        sidecar.shutdown(Duration::from_secs(5)).await.unwrap();
        // Reload persisted readiness into a fresh backend and a fresh dispatcher owner.
        let restarted = SidecarHandle::start(SidecarConfig::new(backend, config))
            .await
            .unwrap();
        let api = WorkbenchApi::connect(restarted.endpoint(), workspace.id);
        let loaded = api.session_generated(&id).await.unwrap();
        assert!(!loaded.is_processing);
        let scheduler = policy::Scheduler::default();
        let flight = scheduler.request(&id).unwrap();
        assert!(scheduler.request(&id).is_none());
        let outcome = drain(&api, &id, "approved", "additional", flight)
            .await
            .unwrap();
        assert!(matches!(outcome, Outcome::None | Outcome::Sent { .. }));
        let final_session = wait_idle(&api, &id).await;
        assert_eq!(
            final_session
                .messages
                .iter()
                .filter(|m| m.role == g::MessageRole::User
                    && m.content.contains("persisted user context"))
                .count(),
            1
        );
        assert!(api.pending_plan(&id).await.unwrap().is_none());
        assert!(matches!(
            dispatch(&api, &id, "approved", "additional").await.unwrap(),
            Outcome::None
        ));
        assert_eq!(
            api.session_generated(&id).await.unwrap().permission_mode,
            Some(g::PermissionMode::AllowAll)
        );
        restarted.shutdown(Duration::from_secs(5)).await.unwrap();
    });
}
#[cfg(test)]
async fn wait_idle(api: &WorkbenchApi, id: &str) -> g::Session {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(40);
    loop {
        let s = api.session_generated(id).await.unwrap();
        if !s.is_processing
            && s.messages
                .iter()
                .any(|m| m.role == g::MessageRole::Assistant)
        {
            return s;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "real plan session did not become idle"
        );
        smol::Timer::after(std::time::Duration::from_millis(30)).await;
    }
}
