#![allow(clippy::unwrap_used)]
mod support;
use agentkit_client::{ClientOptions, ConnectionState, RpcClient, RpcError};
use serde_json::{Value, json};
use std::time::Duration;
use support::*;
#[test]
fn explicitly_idempotent_reads_retry_timeout_once_but_writes_never_retry() {
    let (urls, recv) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            urls.send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            send(&mut ws,json!({"type":"handshake_ack","clientId":"client","registeredChannels":["ready","read","write"]})).await;
            let ready = frame(&mut ws).await;
            assert_eq!(ready["channel"], "ready");
            send(
                &mut ws,
                json!({"type":"response","id":ready["id"],"result":true}),
            )
            .await;
            let first = frame(&mut ws).await;
            assert_eq!(first["channel"], "read");
            let second = frame(&mut ws).await;
            assert_eq!(second["channel"], "read");
            assert_ne!(first["id"], second["id"]);
            // A late reply for the expired id must not complete the retry.
            send(
                &mut ws,
                json!({"type":"response","id":first["id"],"result":99}),
            )
            .await;
            send(
                &mut ws,
                json!({"type":"response","id":second["id"],"result":7}),
            )
            .await;
            assert_eq!(frame(&mut ws).await["channel"], "write");
            while let Some(Ok(message)) = bounded(ws.next()).await.unwrap() {
                if message.is_text() {
                    let req: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
                    assert_ne!(req["type"], "request", "write was retried");
                }
                if message.is_close() {
                    break;
                }
            }
        })
    });
    let mut options =
        ClientOptions::local(recv.recv().unwrap(), "secret".into(), "workspace".into());
    options.request_timeout = Duration::from_millis(250);
    let client = RpcClient::connect(options);
    smol::block_on(async {
        assert!(
            client
                .invoke_timeout::<bool>("ready", vec![], Duration::from_secs(4))
                .await
                .unwrap()
        );
        assert_eq!(client.invoke_read::<u64>("read", vec![]).await.unwrap(), 7);
        let error = client.invoke::<Value>("write", vec![]).await.unwrap_err();
        assert_eq!(error.protocol_code(), "CLIENT_REQUEST_TIMEOUT");
    });
    client.close();
    server.join().unwrap();
}
#[test]
fn server_shutdown_event_stops_reconnect() {
    let (urls, recv) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            urls.send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            send(
                &mut ws,
                json!({"type":"handshake_ack","clientId":"client","registeredChannels":[]}),
            )
            .await;
            send(
                &mut ws,
                json!({"type":"event","channel":"server:shuttingDown","seq":1,"args":[]}),
            )
            .await;
            let _ = bounded(ws.next()).await;
        })
    });
    let client = RpcClient::connect(ClientOptions::local(
        recv.recv().unwrap(),
        "secret".into(),
        "workspace".into(),
    ));
    let events = client.subscribe("server:shuttingDown");
    let states = client.state_changes();
    smol::block_on(async {
        bounded(events.recv()).await.unwrap().unwrap();
        loop {
            let state = bounded(states.recv()).await.unwrap().unwrap();
            if state == ConnectionState::Disconnected {
                break;
            }
            assert!(!matches!(state, ConnectionState::Reconnecting { .. }));
        }
    });
    client.close();
    server.join().unwrap();
}
#[test]
fn error_codes_and_debug_keep_secrets_out_of_diagnostics() {
    let options = ClientOptions::local(
        "ws://127.0.0.1:1".into(),
        "do-not-print-this".into(),
        "workspace".into(),
    );
    assert!(!format!("{options:?}").contains("do-not-print-this"));
    assert_eq!(
        RpcError::Disconnected.protocol_code(),
        "CLIENT_DISCONNECTED"
    );
    assert_eq!(RpcError::TooLarge.protocol_code(), "TRANSFER_TOO_LARGE");
}
use futures::StreamExt;

#[test]
fn expired_queued_write_never_runs_after_handshake_completes() {
    let (urls, recv) = std::sync::mpsc::channel();
    let (release, wait) = async_channel::bounded::<()>(1);
    let server = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            urls.send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            bounded(wait.recv()).await.unwrap().unwrap();
            send(
                &mut ws,
                json!({"type":"handshake_ack","clientId":"client","registeredChannels":["write"]}),
            )
            .await;
            let req = frame(&mut ws).await;
            assert_eq!(req["args"][0], "fresh");
            send(
                &mut ws,
                json!({"type":"response","id":req["id"],"result":true}),
            )
            .await;
        })
    });
    let client = RpcClient::connect(ClientOptions::local(
        recv.recv().unwrap(),
        "secret".into(),
        "workspace".into(),
    ));
    smol::block_on(async {
        assert!(matches!(
            client
                .invoke_timeout::<Value>("write", vec![json!("expired")], Duration::from_millis(30))
                .await,
            Err(RpcError::Timeout)
        ));
        release.send(()).await.unwrap();
        assert!(
            client
                .invoke::<bool>("write", vec![json!("fresh")])
                .await
                .unwrap()
        );
    });
    client.close();
    server.join().unwrap();
}

#[test]
fn typed_session_stream_reports_bad_known_payload_and_keeps_future_event() {
    let (urls, recv) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            urls.send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            send(
                &mut ws,
                json!({"type":"handshake_ack","clientId":"client","registeredChannels":[]}),
            )
            .await;
            for (seq, event) in [
                json!({"type":"text_delta","sessionId":"s"}),
                json!({"type":"new_future","sessionId":"s","newField":[1,2]}),
            ]
            .into_iter()
            .enumerate()
            {
                send(
                    &mut ws,
                    json!({"type":"event","channel":"session:event","seq":seq+1,"args":[event]}),
                )
                .await;
            }
            let _ = bounded(ws.next()).await;
        })
    });
    let client = RpcClient::connect(ClientOptions::local(
        recv.recv().unwrap(),
        "secret".into(),
        "workspace".into(),
    ));
    let events = client.session_events();
    smol::block_on(async {
        assert!(bounded(events.recv()).await.unwrap().unwrap().is_err());
        let future = bounded(events.recv()).await.unwrap().unwrap().unwrap();
        assert!(future.known().is_none());
        assert_eq!(future.raw()["newField"], json!([1, 2]));
    });
    client.close();
    server.join().unwrap();
}

#[test]
fn shutdown_notice_allows_an_explicit_sidecar_endpoint_replacement() {
    let (send_url, receive_url) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        smol::block_on(async {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            send_url
                .send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            send(
                &mut ws,
                json!({"type":"handshake_ack","clientId":"old","registeredChannels":[]}),
            )
            .await;
            send(
                &mut ws,
                json!({"type":"event","channel":"server:shuttingDown","seq":1,"args":[]}),
            )
            .await;
            let _ = bounded(ws.next()).await;
            let (tcp, _) = bounded(listener.accept()).await.unwrap().unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let handshake = frame(&mut ws).await;
            assert!(handshake.get("reconnectClientId").is_none());
            send(
                &mut ws,
                json!({"type":"handshake_ack","clientId":"new","registeredChannels":["echo"]}),
            )
            .await;
            send(
                &mut ws,
                json!({"type":"event","channel":"after-replace","seq":1,"args":["recovered"]}),
            )
            .await;
            let request = frame(&mut ws).await;
            send(
                &mut ws,
                json!({"type":"response","id":request["id"],"result":42}),
            )
            .await;
        })
    });
    let url = receive_url.recv().unwrap();
    let client = RpcClient::connect(ClientOptions::local(
        url.clone(),
        "old".into(),
        "workspace".into(),
    ));
    let states = client.state_changes();
    smol::block_on(async {
        loop {
            if bounded(states.recv()).await.unwrap().unwrap() == ConnectionState::Disconnected {
                break;
            }
        }
        // A consumer may begin observing while the backend is intentionally stopped.
        let states = client.state_changes();
        assert_eq!(
            bounded(states.recv()).await.unwrap().unwrap(),
            ConnectionState::Disconnected
        );
        let events = client.subscribe("after-replace");
        client.replace_endpoint(agentkit_client::Endpoint {
            url,
            token: "new".into(),
        });
        loop {
            if bounded(states.recv()).await.unwrap().unwrap() == ConnectionState::Connected {
                break;
            }
        }
        assert_eq!(client.invoke::<u32>("echo", vec![]).await.unwrap(), 42);
        assert_eq!(
            bounded(events.recv()).await.unwrap().unwrap(),
            vec![json!("recovered")]
        );
    });
    client.close();
    server.join().unwrap();
}
