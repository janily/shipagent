#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Explicit real-process contracts. Ordinary cargo test skips these; CI `contract` runs them.
mod contract_support;
use agentkit_client::{ConnectionState, RpcClient, RpcError};
use agentkit_protocol::{ErrorCode, RpcValue, WireError};
use contract_support::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Info {
    online: u32,
    writes: u32,
    buffer_limit: u32,
}
async fn info(client: &RpcClient) -> Info {
    deadline(client.invoke("contract:info", vec![]))
        .await
        .unwrap()
}
async fn online(client: &RpcClient, count: u32) {
    deadline(async {
        while info(client).await.online != count {
            smol::Timer::after(Duration::from_millis(10)).await;
        }
    })
    .await
}

#[test]
#[ignore = "Requires built backend and --contract fixture; see tools/contract/README.md"]
fn upstream_transport_auth_binary_capabilities_and_deadlines() {
    let fixture = Fixture::start();
    smol::block_on(deadline(async {
        let mut bad = fixture.options("auth");
        bad.endpoint.token = "wrong isolated fixture token".into();
        let rejected = RpcClient::connect(bad);
        let error = rejected
            .invoke::<Value>("contract:info", vec![])
            .await
            .unwrap_err();
        assert_eq!(error.protocol_code(), "AUTH_FAILED");
        until(|| matches!(rejected.state(), ConnectionState::Fatal { .. })).await;
        rejected.close();

        let mut options = fixture.options("auth");
        options.capability_timeout = Duration::from_millis(40);
        options.capabilities.insert(
            "client:contractEcho".into(),
            Arc::new(|mut args| Box::pin(async move { Ok(args.remove(0)) })),
        );
        options.capabilities.insert(
            "client:contractFailure".into(),
            Arc::new(|_| {
                Box::pin(async {
                    Err(WireError {
                        code: ErrorCode::HandlerError,
                        message: "expected capability failure".into(),
                        data: None,
                    })
                })
            }),
        );
        options.capabilities.insert(
            "client:contractSlow".into(),
            Arc::new(|_| {
                Box::pin(async {
                    smol::Timer::after(Duration::from_millis(250)).await;
                    Ok(RpcValue::Null)
                })
            }),
        );
        options.capabilities.insert(
            "client:browser:invoke".into(),
            Arc::new(|_| {
                Box::pin(async { panic!("unimplemented browser capability must never run") })
            }),
        );
        let client = RpcClient::connect(options);
        assert_eq!(info(&client).await.writes, 0);
        let snapshot = client.snapshot();
        assert_eq!(
            snapshot.server_version.as_deref(),
            Some("shipagent-contract-v0.2.6")
        );
        assert!(snapshot.client_id.is_some());
        assert!(snapshot.max_message_bytes > 0);
        assert!(
            snapshot
                .registered_channels
                .unwrap()
                .iter()
                .any(|v| v == "contract:reverse")
        );

        let nested = RpcValue::Object(
            [(
                "array".into(),
                RpcValue::Array(vec![
                    RpcValue::Bytes(vec![0, 1, 127, 255]),
                    RpcValue::Bytes(vec![]),
                ]),
            )]
            .into(),
        );
        assert_eq!(
            client
                .invoke_values("contract:echo", vec![nested.clone()])
                .await
                .unwrap(),
            nested
        );
        assert_eq!(
            client
                .invoke_values(
                    "contract:reverse",
                    vec![
                        RpcValue::String("client:contractEcho".into()),
                        nested.clone()
                    ]
                )
                .await
                .unwrap(),
            nested
        );
        let error = client
            .invoke::<Value>(
                "contract:reverse",
                vec![json!("client:contractFailure"), Value::Null],
            )
            .await
            .unwrap_err();
        assert_eq!(error.protocol_code(), "HANDLER_ERROR");
        for channel in ["client:missing", "client:browser:invoke"] {
            let error = client
                .invoke::<Value>("contract:reverse", vec![json!(channel), Value::Null])
                .await
                .unwrap_err();
            assert_eq!(error.protocol_code(), "CAPABILITY_UNAVAILABLE");
        }
        let (slow, echo) = futures::join!(
            client.invoke::<Value>(
                "contract:reverse",
                vec![json!("client:contractSlow"), Value::Null]
            ),
            client.invoke::<String>(
                "contract:echo",
                vec![json!("foreground remains responsive")]
            ),
        );
        assert_eq!(slow.unwrap_err().protocol_code(), "CLIENT_REQUEST_TIMEOUT");
        assert_eq!(echo.unwrap(), "foreground remains responsive");
        assert_eq!(
            client
                .invoke::<Value>("contract:fail", vec![])
                .await
                .unwrap_err()
                .protocol_code(),
            "HANDLER_ERROR"
        );
        assert_eq!(
            client
                .invoke::<Value>("not:registered", vec![])
                .await
                .unwrap_err()
                .protocol_code(),
            "CHANNEL_NOT_FOUND"
        );

        assert!(matches!(
            client
                .invoke_timeout::<String>(
                    "contract:slow",
                    vec![json!(150), json!("late")],
                    Duration::from_millis(25)
                )
                .await,
            Err(RpcError::Timeout)
        ));
        assert_eq!(
            client
                .invoke::<String>("contract:echo", vec![json!("fresh")])
                .await
                .unwrap(),
            "fresh"
        );
        smol::Timer::after(Duration::from_millis(180)).await;
        assert_eq!(
            client
                .invoke::<String>("contract:echo", vec![json!("after late response")])
                .await
                .unwrap(),
            "after late response"
        );
        let huge = "x".repeat(client.snapshot().max_message_bytes as usize);
        assert!(matches!(
            client
                .invoke::<Value>("contract:echo", vec![json!(huge)])
                .await,
            Err(RpcError::TooLarge)
        ));
        client.close();
    }));
}

#[test]
#[ignore = "Requires built backend and --contract fixture; see tools/contract/README.md"]
fn upstream_replays_real_events_marks_eviction_stale_and_never_replays_writes() {
    let fixture = Fixture::start();
    let proxy = FaultProxy::start(fixture.endpoint.url.clone());
    let mut options = fixture.options("recovery");
    options.endpoint.url = proxy.url.clone();
    let client = RpcClient::connect(options);
    let observer = fixture.client("recovery");
    let events = client.subscribe("contract:event");
    smol::block_on(deadline(async {
        info(&client).await;
        online(&observer, 2).await;
        observer
            .invoke::<()>("contract:push", vec![json!([0])])
            .await
            .unwrap();
        assert_eq!(deadline(events.recv()).await.unwrap(), vec![json!(0)]);
        let first = client.snapshot();
        let old_bound = client.bind_connection().unwrap().0;
        let (pending, ()) =
            futures::join!(client.invoke::<u32>("contract:commit", vec![]), async {
                while info(&observer).await.writes != 1 {
                    smol::Timer::after(Duration::from_millis(5)).await;
                }
                proxy.pause().await;
                online(&observer, 1).await;
            });
        assert_eq!(pending.unwrap_err().protocol_code(), "CLIENT_DISCONNECTED");
        observer
            .invoke::<()>("contract:releaseWrites", vec![])
            .await
            .unwrap();
        observer
            .invoke::<()>("contract:push", vec![json!([1, 2, 3])])
            .await
            .unwrap();
        proxy.resume().await;
        until(|| client.snapshot().recovery_id > first.recovery_id).await;
        assert_eq!(client.snapshot().client_id, first.client_id);
        assert!(!client.snapshot().needs_refresh);
        for value in 1..=3 {
            assert_eq!(deadline(events.recv()).await.unwrap(), vec![json!(value)]);
        }
        assert!(client.snapshot().last_seen_seq > first.last_seen_seq);
        assert_eq!(info(&observer).await.writes, 1, "old mutation was replayed");
        assert_eq!(
            old_bound
                .invoke::<u32>("contract:commit", vec![])
                .await
                .unwrap_err()
                .protocol_code(),
            "CLIENT_DISCONNECTED"
        );
        assert_eq!(info(&observer).await.writes, 1);

        let resumed = client.snapshot();
        proxy.pause().await;
        online(&observer, 1).await;
        let count = info(&observer).await.buffer_limit + 2;
        observer
            .invoke::<()>("contract:push", vec![json!((0..count).collect::<Vec<_>>())])
            .await
            .unwrap();
        proxy.resume().await;
        until(|| client.snapshot().recovery_id > resumed.recovery_id).await;
        let stale = client.snapshot();
        assert_eq!(stale.client_id, first.client_id);
        assert!(
            stale.needs_refresh,
            "real upstream eviction must force snapshot refresh"
        );
        assert_eq!(stale.refresh_epoch, stale.recovery_id);
        assert_eq!(info(&client).await.writes, 1);
        observer
            .invoke::<()>("contract:push", vec![json!([9999])])
            .await
            .unwrap();
        assert_eq!(
            deadline(events.recv()).await.unwrap(),
            vec![json!(9999)],
            "stale buffer must not be replayed as complete history"
        );
        client.close();
        observer.close();
    }));
}

#[test]
#[ignore = "Requires actual bundled backend; AGENTKIT_BACKEND_DIR"]
fn bundled_sidecar_session_draft_attachment_and_event_contracts() {
    use agentkit_client::ClientOptions;
    use agentkit_protocol::generated as g;
    use agentkit_sidecar::{SidecarConfig, SidecarHandle};
    let profile = tempfile::tempdir().unwrap();
    let backend = std::env::var_os("AGENTKIT_BACKEND_DIR").expect("set AGENTKIT_BACKEND_DIR");
    smol::block_on(deadline(async {
        let sidecar = SidecarHandle::start(SidecarConfig::new(
            backend.into(),
            profile.path().join("config"),
        ))
        .await
        .unwrap();
        let endpoint = sidecar.endpoint();
        let mut options = ClientOptions::local(endpoint.url, endpoint.token, String::new());
        options.capabilities.insert(
            "client:openFileDialog".into(),
            Arc::new(|args| {
                Box::pin(async move {
                    assert_eq!(args.len(), 1);
                    let spec = serde_json::to_value(&args[0]).unwrap();
                    assert_eq!(spec["properties"], json!(["openFile", "multiSelections"]));
                    Ok(serde_json::from_value(
                        json!({"canceled":false,"filePaths":["contract selection"]}),
                    )
                    .unwrap())
                })
            }),
        );
        let client = RpcClient::connect(options);
        client.service().health().await.unwrap();
        use agentkit_protocol::constants::channels as c;
        let selection: Vec<String> = client.invoke(c::file::OPEN_DIALOG, vec![]).await.unwrap();
        assert_eq!(selection, vec!["contract selection"]);
        client.handle_capability(
            "client:openFileDialog",
            Arc::new(|_| {
                Box::pin(async {
                    Err(WireError {
                        code: ErrorCode::HandlerError,
                        message: "expected dialog failure".into(),
                        data: None,
                    })
                })
            }),
        );
        assert_eq!(
            client
                .invoke::<Value>(c::file::OPEN_DIALOG, vec![])
                .await
                .unwrap_err()
                .protocol_code(),
            "HANDLER_ERROR"
        );
        let workspace = client.workspaces().list().await.unwrap().remove(0);
        client.workspaces().switch(&workspace.id).await.unwrap();
        let events = client.events().session();
        let session = client.sessions().create(&workspace.id, None).await.unwrap();
        assert!(
            client
                .sessions()
                .list()
                .await
                .unwrap()
                .iter()
                .any(|s| s.id == session.id)
        );
        client
            .drafts()
            .set(
                &session.id,
                &g::SessionDraft {
                    text: "contract 草稿".into(),
                    attachments: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(
            client
                .drafts()
                .get(&session.id)
                .await
                .unwrap()
                .unwrap()
                .text,
            "contract 草稿"
        );
        client.drafts().delete(&session.id).await.unwrap();
        assert!(client.drafts().get(&session.id).await.unwrap().is_none());
        let attachment: g::FileAttachment = serde_json::from_value(json!({"type":"text","path":"proof.txt","name":"proof.txt","mimeType":"text/plain","size":16,"text":"contract bytes\n"})).unwrap();
        let stored = client
            .files()
            .store_attachment(&session.id, &attachment)
            .await
            .unwrap();
        assert_eq!(stored.name, "proof.txt");
        let loaded = client
            .files()
            .read_attachment(&stored.stored_path)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.text.as_deref(), Some("contract bytes\n"));
        assert_eq!(
            client
                .invoke_values(
                    c::file::READ_BINARY,
                    vec![RpcValue::String(stored.stored_path.clone())]
                )
                .await
                .unwrap(),
            RpcValue::Bytes(b"contract bytes\n".to_vec())
        );
        client
            .sessions()
            .command(
                &session.id,
                &g::SessionCommand::Rename {
                    name: "contract renamed".into(),
                },
            )
            .await
            .unwrap();
        deadline(async {
            loop {
                let event = events.recv().await.unwrap().unwrap();
                let raw = serde_json::to_value(event).unwrap();
                if raw["type"] == "name_changed" && raw["sessionId"] == session.id {
                    break;
                }
            }
        })
        .await;
        assert_eq!(
            client
                .sessions()
                .get_messages(&session.id)
                .await
                .unwrap()
                .unwrap()
                .name
                .as_deref(),
            Some("contract renamed")
        );
        client.sessions().delete(&session.id).await.unwrap();
        assert!(
            client
                .sessions()
                .get_messages(&session.id)
                .await
                .unwrap()
                .is_none()
        );
        client.close();
        sidecar.shutdown(Duration::from_secs(3)).await.unwrap();
    }));
}
