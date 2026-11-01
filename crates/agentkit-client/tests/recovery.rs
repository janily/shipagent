#![allow(clippy::unwrap_used)]
use agentkit_client::{ClientOptions, RpcClient, RpcError};
use futures::StreamExt;
use serde_json::{Value, json};
use std::time::Duration;
mod support;
use support::*;
#[test]
fn reconnect_keeps_subscribers_and_rejects_old_writes_without_replaying() {
    let (port_send, port_recv) = std::sync::mpsc::channel();
    let (lost, seen_loss) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            port_send
                .send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            send(&mut ws,json!({"type":"handshake_ack","clientId":"old","registeredChannels":["write","echo"]})).await;
            send(
                &mut ws,
                json!({"type":"event","channel":"event","seq":7,"args":["before"]}),
            )
            .await;
            assert_eq!(frame(&mut ws).await["channel"], "write");
            ws.close(None).await.unwrap();
            drop(ws);
            lost.send(()).unwrap();
            let Some(Ok((tcp, _))) = bounded(listener.accept()).await else {
                return false;
            };
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let h = frame(&mut ws).await;
            assert_eq!(h["reconnectClientId"], "old");
            assert_eq!(h["lastSeq"], 7);
            send(&mut ws,json!({"type":"handshake_ack","clientId":"old","reconnected":true,"stale":false,"registeredChannels":["echo"]})).await;
            send(
                &mut ws,
                json!({"type":"event","channel":"event","seq":8,"args":["after"]}),
            )
            .await;
            let req = frame(&mut ws).await;
            assert_eq!(req["channel"], "echo");
            send(
                &mut ws,
                json!({"type":"response","id":req["id"],"result":42}),
            )
            .await;
            true
        })
    });
    let client = RpcClient::connect(ClientOptions::local(
        port_recv.recv().unwrap(),
        "secret".into(),
        "workspace".into(),
    ));
    let events = client.subscribe("event");
    let first = smol::block_on(bounded(events.recv())).unwrap().unwrap();
    assert_eq!(first[0], "before");
    let write = smol::block_on(client.invoke::<Value>("write", vec![]));
    seen_loss.recv_timeout(Duration::from_secs(2)).unwrap();
    // Existing prototype has no reconnection; this call must wait for recovery.
    let result =
        smol::block_on(client.invoke_timeout::<u64>("echo", vec![], Duration::from_secs(3)));
    let after = smol::block_on(bounded(events.recv()));
    client.close();
    let reconnected = server.join().unwrap();
    assert!(matches!(write, Err(RpcError::Disconnected)));
    assert!(reconnected, "server never received reconnect handshake");
    assert_eq!(result.unwrap(), 42);
    assert_eq!(after.unwrap().unwrap()[0], "after");
}

#[test]
fn malformed_binary_is_rejected_even_when_caller_requests_json() {
    let (port_send, port_recv) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            port_send
                .send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            send(
                &mut ws,
                json!({"type":"handshake_ack","clientId":"old","registeredChannels":["echo"]}),
            )
            .await;
            let req = frame(&mut ws).await;
            send(&mut ws,json!({"type":"response","id":req["id"],"result":{"nested":{"__craftRpcType":"u8","base64":"!bad"}}})).await;
        })
    });
    let client = RpcClient::connect(ClientOptions::local(
        port_recv.recv().unwrap(),
        "secret".into(),
        "workspace".into(),
    ));
    let reply = smol::block_on(client.invoke::<Value>("echo", vec![]));
    client.close();
    server.join().unwrap();
    assert!(
        reply.is_err(),
        "invalid binary wrapper escaped wire validation"
    );
}

#[test]
fn recovery_classifies_stale_fresh_and_resumed_connections_and_keeps_max_seq() {
    let (port_send, port_recv) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            port_send
                .send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            for index in 0..5 {
                let (tcp, _) = bounded(listener.accept()).await.unwrap().unwrap();
                let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
                let h = frame(&mut ws).await;
                if index > 0 {
                    assert_eq!(h["reconnectClientId"], "client");
                    assert_eq!(h["lastSeq"], 12);
                }
                let mut ack = json!({"type":"handshake_ack","clientId":"client","registeredChannels":["drop","echo","pull"],"serverVersion":format!("v{index}")});
                if index == 1 || index == 2 || index == 4 {
                    ack["reconnected"] = json!(true);
                    ack["stale"] = json!(index == 2);
                }
                send(&mut ws, ack).await;
                let echo = frame(&mut ws).await;
                assert_eq!(echo["channel"], "echo");
                send(
                    &mut ws,
                    json!({"type":"response","id":echo["id"],"result":index}),
                )
                .await;
                if index == 4 {
                    let _ = bounded(ws.next()).await;
                    break;
                }
                let gate = frame(&mut ws).await;
                assert_eq!(gate["channel"], "pull");
                for seq in [9, 6, 12] {
                    send(
                        &mut ws,
                        json!({"type":"event","channel":"event","seq":seq,"args":[seq]}),
                    )
                    .await;
                }
                send(
                    &mut ws,
                    json!({"type":"response","id":gate["id"],"result":true}),
                )
                .await;
                assert_eq!(frame(&mut ws).await["channel"], "drop");
                ws.close(None).await.unwrap();
                if index == 0 {
                    // Failed reconnect before ack must not erase the last good cursor.
                    let (tcp, _) = bounded(listener.accept()).await.unwrap().unwrap();
                    let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
                    let h = frame(&mut ws).await;
                    assert_eq!(h["lastSeq"], 12);
                    assert_eq!(h["reconnectClientId"], "client");
                    ws.close(None).await.unwrap();
                }
            }
        })
    });
    let mut opts = ClientOptions::local(
        port_recv.recv().unwrap(),
        "secret".into(),
        "workspace".into(),
    );
    opts.max_reconnect_delay = Duration::from_millis(20);
    let client = RpcClient::connect(opts);
    let events = client.subscribe("event");
    let recovery = client.subscribe("__transport:reconnected");
    smol::block_on(async {
        for index in 0..5 {
            // A failed handshake can reject a queued unsent read; explicitly retry
            // after observing its Reconnecting state, never replay any write.
            let mut result = client.invoke::<u64>("echo", vec![]).await;
            if index == 1 && result.is_err() {
                result = client.invoke("echo", vec![]).await;
            }
            assert_eq!(result.unwrap(), index);
            assert_eq!(client.server_version(), Some(format!("v{index}")));
            if index > 0 {
                let notice = bounded(recovery.recv()).await.unwrap().unwrap();
                assert_eq!(notice[0], index == 2 || index == 3);
                assert_eq!(
                    client.snapshot().last_seen_seq,
                    if index == 3 { 0 } else { 12 }
                );
            }
            assert_eq!(
                client.snapshot().refresh_epoch,
                match index {
                    0 | 1 => 1,
                    2 => 3,
                    _ => 4,
                }
            );
            if index == 4 {
                break;
            }
            assert!(client.invoke::<bool>("pull", vec![]).await.unwrap());
            for seq in [9, 6, 12] {
                assert_eq!(bounded(events.recv()).await.unwrap().unwrap()[0], seq);
            }
            assert!(matches!(
                client.invoke::<Value>("drop", vec![]).await,
                Err(RpcError::Disconnected)
            ));
        }
    });
    client.close();
    server.join().unwrap();
}

#[test]
fn endpoint_replacement_clears_cursor_token_and_advertised_metadata() {
    let (urls, recv) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        smol::block_on(async move {
            let old = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let new = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            urls.send((
                format!("ws://{}", old.local_addr().unwrap()),
                format!("ws://{}", new.local_addr().unwrap()),
            ))
            .unwrap();
            let (tcp, _) = old.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            send(&mut ws,json!({"type":"handshake_ack","clientId":"old","registeredChannels":["drop"],"maxMessageBytes":1024,"serverVersion":"old"})).await;
            send(
                &mut ws,
                json!({"type":"event","channel":"event","seq":20,"args":[]}),
            )
            .await;
            let _ = frame(&mut ws).await;
            ws.close(None).await.unwrap();
            let (tcp, _) = bounded(new.accept()).await.unwrap().unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let h = frame(&mut ws).await;
            assert_eq!(h["token"], "new-secret");
            assert!(h.get("reconnectClientId").is_none());
            assert!(h.get("lastSeq").is_none());
            send(&mut ws,json!({"type":"handshake_ack","clientId":"new","registeredChannels":["echo"],"serverVersion":"new","maxMessageBytes":256})).await;
            let req = frame(&mut ws).await;
            assert_eq!(req["channel"], "echo");
            send(
                &mut ws,
                json!({"type":"response","id":req["id"],"result":true}),
            )
            .await;
        })
    });
    let (old, new) = recv.recv().unwrap();
    let client = RpcClient::connect(ClientOptions::local(
        old,
        "old-secret".into(),
        "workspace".into(),
    ));
    let recovery = client.subscribe("__transport:reconnected");
    assert!(smol::block_on(client.invoke::<Value>("drop", vec![])).is_err());
    client.replace_endpoint(agentkit_client::Endpoint {
        url: new,
        token: "new-secret".into(),
    });
    assert_eq!(
        smol::block_on(bounded(recovery.recv())).unwrap().unwrap()[0],
        true
    );
    assert!(!client.has_channel("drop"));
    assert_eq!(client.server_version().as_deref(), Some("new"));
    assert_eq!(client.snapshot().max_message_bytes, 256);
    assert!(matches!(
        smol::block_on(client.invoke::<Value>("echo", vec![json!("界".repeat(200))])),
        Err(RpcError::TooLarge)
    ));
    assert!(smol::block_on(client.invoke::<bool>("echo", vec![])).unwrap());
    client.close();
    server.join().unwrap();
}

#[test]
fn close_interrupts_backoff_and_state_observers_finish() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    drop(listener);
    let client = RpcClient::connect(ClientOptions::local(
        url,
        "secret".into(),
        "workspace".into(),
    ));
    let states = client.state_changes();
    smol::block_on(async {
        loop {
            if matches!(
                bounded(states.recv()).await.unwrap().unwrap(),
                agentkit_client::ConnectionState::Reconnecting { .. }
            ) {
                break;
            }
        }
        let start = std::time::Instant::now();
        client.close();
        loop {
            if bounded(states.recv()).await.unwrap().unwrap()
                == agentkit_client::ConnectionState::Disconnected
            {
                break;
            }
        }
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(bounded(states.recv()).await.unwrap().is_err());
    });
}

#[test]
fn workspace_binding_changes_only_after_successful_switch_response() {
    let (urls, recv) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            urls.send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            assert_eq!(frame(&mut ws).await["workspaceId"], "workspace");
            send(&mut ws,json!({"type":"handshake_ack","clientId":"client","registeredChannels":["window:switchWorkspace","drop"]})).await;
            let bad = frame(&mut ws).await;
            send(&mut ws,json!({"type":"error","id":bad["id"],"error":{"code":"HANDLER_ERROR","message":"denied"}})).await;
            let good = frame(&mut ws).await;
            assert_eq!(good["args"][0], "new-workspace");
            send(
                &mut ws,
                json!({"type":"response","id":good["id"],"result":null}),
            )
            .await;
            let _ = frame(&mut ws).await;
            ws.close(None).await.unwrap();
            let (tcp, _) = bounded(listener.accept()).await.unwrap().unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            assert_eq!(frame(&mut ws).await["workspaceId"], "new-workspace");
            send(
                &mut ws,
                json!({"type":"handshake_ack","clientId":"client","registeredChannels":["echo"]}),
            )
            .await;
            let req = frame(&mut ws).await;
            send(
                &mut ws,
                json!({"type":"response","id":req["id"],"result":true}),
            )
            .await;
        })
    });
    let mut options =
        ClientOptions::local(recv.recv().unwrap(), "secret".into(), "workspace".into());
    options.max_reconnect_delay = Duration::from_millis(20);
    let client = RpcClient::connect(options);
    smol::block_on(async {
        assert!(client.switch_workspace("denied").await.is_err());
        client.switch_workspace("new-workspace").await.unwrap();
        assert!(client.invoke::<Value>("drop", vec![]).await.is_err());
        assert!(client.invoke::<bool>("echo", vec![]).await.unwrap());
    });
    client.close();
    server.join().unwrap();
}

#[test]
fn deferred_bound_write_cannot_migrate_to_another_connection() {
    let (urls, recv) = std::sync::mpsc::channel();
    let (resume, waiting) = async_channel::bounded::<()>(1);
    let server = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            urls.send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            send(&mut ws,json!({"type":"handshake_ack","clientId":"old","registeredChannels":["echo","drop","write"]})).await;
            let ready = frame(&mut ws).await;
            send(
                &mut ws,
                json!({"type":"response","id":ready["id"],"result":true}),
            )
            .await;
            let _ = frame(&mut ws).await;
            ws.close(None).await.unwrap();
            let (tcp, _) = bounded(listener.accept()).await.unwrap().unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            bounded(waiting.recv()).await.unwrap().unwrap();
            send(&mut ws,json!({"type":"handshake_ack","clientId":"new","stale":true,"registeredChannels":["echo","write"]})).await;
            let req = frame(&mut ws).await;
            let old_was_sent = req["channel"] == "write";
            send(
                &mut ws,
                json!({"type":"response","id":req["id"],"result":true}),
            )
            .await;
            old_was_sent
        })
    });
    let mut options =
        ClientOptions::local(recv.recv().unwrap(), "secret".into(), "workspace".into());
    options.max_reconnect_delay = Duration::from_millis(20);
    let client = RpcClient::connect(options);
    let recovery = client.subscribe("__transport:reconnected");
    assert!(smol::block_on(client.invoke::<bool>("echo", vec![])).unwrap());
    let (bound, _snapshot) = client.bind_connection().unwrap();
    let deferred = bound.invoke_timeout::<bool>("write", vec![], Duration::from_secs(1));
    assert!(smol::block_on(client.invoke::<Value>("drop", vec![])).is_err());
    assert!(
        matches!(client.bind_connection(), Err(RpcError::Disconnected)),
        "lost socket cannot issue a new binding"
    );
    resume.try_send(()).unwrap();
    smol::block_on(bounded(recovery.recv())).unwrap().unwrap();
    let late = smol::block_on(deferred);
    let good = if late.is_err() {
        Some(smol::block_on(client.invoke::<bool>("echo", vec![])))
    } else {
        None
    };
    client.close();
    assert!(!server.join().unwrap(), "old intent was sent on new socket");
    assert!(matches!(late, Err(RpcError::Disconnected)));
    assert!(good.unwrap().unwrap());
}
