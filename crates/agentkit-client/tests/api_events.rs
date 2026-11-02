#![allow(clippy::unwrap_used)]
mod support;
use agentkit_client::{ClientOptions, RpcClient};
use serde_json::json;
use support::*;
#[test]
fn all_v1_event_streams_validate_arguments_and_preserve_undefined_lifecycle_payloads() {
    let (urls, recv) = std::sync::mpsc::channel();
    let (ready, wait) = async_channel::bounded::<()>(1);
    let server = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            urls.send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            wait.recv().await.unwrap();
            send(
                &mut ws,
                json!({"type":"handshake_ack","clientId":"events","registeredChannels":[]}),
            )
            .await;
            let records = [
                (
                    "session:event",
                    json!([{"type":"future","sessionId":"s","extra":true}]),
                ),
                (
                    "sessions:unreadSummaryChanged",
                    json!([{"totalUnreadSessions":0,"byWorkspace":{},"hasUnreadByWorkspace":{}}]),
                ),
                ("sessions:filesChanged", json!([42])),
                ("sessions:filesChanged", json!(["s"])),
                ("skills:changed", json!(["w", []])),
                ("LLM_Connection:changed", json!([])),
                ("permissions:defaultsChanged", json!([null])),
                ("server:statusChanged", json!([{"future":true}])),
                ("server:shuttingDown", json!([])),
            ];
            for (i, (channel, args)) in records.into_iter().enumerate() {
                send(
                    &mut ws,
                    json!({"type":"event","channel":channel,"seq":i+1,"args":args}),
                )
                .await;
            }
        })
    });
    let client = RpcClient::connect(ClientOptions::local(
        recv.recv().unwrap(),
        "key".into(),
        "w".into(),
    ));
    let session = client.events().session();
    let unread = client.events().unread_summary();
    let files = client.events().files_changed();
    let skills = client.events().skills_changed();
    let connections = client.events().connections_changed();
    let permissions = client.events().permissions_changed();
    let status = client.events().server_status();
    let shutdown = client.events().server_shutting_down();
    ready.try_send(()).unwrap();
    smol::block_on(async {
        let event = bounded(session.recv()).await.unwrap().unwrap().unwrap();
        assert!(event.known().is_none());
        assert_eq!(event.raw()["extra"], true);
        assert_eq!(
            bounded(unread.recv())
                .await
                .unwrap()
                .unwrap()
                .unwrap()
                .total_unread_sessions,
            0.0
        );
        assert!(bounded(files.recv()).await.unwrap().unwrap().is_err());
        assert_eq!(bounded(files.recv()).await.unwrap().unwrap().unwrap(), "s");
        assert_eq!(
            bounded(skills.recv()).await.unwrap().unwrap().unwrap().0,
            "w"
        );
        bounded(connections.recv()).await.unwrap().unwrap().unwrap();
        bounded(permissions.recv()).await.unwrap().unwrap().unwrap();
        assert_eq!(
            bounded(status.recv())
                .await
                .unwrap()
                .unwrap()
                .unwrap()
                .arguments
                .len(),
            1
        );
        assert!(
            bounded(shutdown.recv())
                .await
                .unwrap()
                .unwrap()
                .unwrap()
                .arguments
                .is_empty()
        );
    });
    client.close();
    server.join().unwrap();
    assert_eq!(agentkit_client::api::V1_EVENT_CHANNELS.len(), 8);
}
