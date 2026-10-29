#![allow(clippy::unwrap_used)]
use agentkit_client::{ClientOptions, RpcClient, RpcError};
use futures::StreamExt;
use serde_json::{Value, json};
use std::time::Duration;

fn server(disconnect: bool, respond: bool) -> (String, std::thread::JoinHandle<()>) {
    let (send, recv) = std::sync::mpsc::channel();
    let task = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            send.send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(stream).await.unwrap();
            let request: Value =
                serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
            assert_eq!(request["type"], "handshake");
            assert_eq!(request["token"], "test-secret");
            assert_eq!(request["workspaceId"], "workspace");
            ws.send(async_tungstenite::tungstenite::Message::Text(json!({"id":request["id"],"type":"handshake_ack","clientId":"client","protocolVersion":"1.0","registeredChannels":["echo"],"maxMessageBytes":1024}).to_string().into())).await.unwrap();
            while let Some(Ok(frame)) = ws.next().await {
                if !frame.is_text() {
                    continue;
                }
                let req: Value = serde_json::from_str(frame.to_text().unwrap()).unwrap();
                if req["type"] != "request" {
                    continue;
                }
                if disconnect {
                    let _ = ws.close(None).await;
                    break;
                }
                if respond {
                    ws.send(async_tungstenite::tungstenite::Message::Text(
                        json!({"id":req["id"],"type":"response","result":req["args"][0]})
                            .to_string()
                            .into(),
                    ))
                    .await
                    .unwrap();
                    ws.send(async_tungstenite::tungstenite::Message::Text(json!({"id":"event","type":"event","channel":"session:event","seq":1,"args":[{"type":"complete","sessionId":"s"}]}).to_string().into())).await.unwrap();
                }
            }
        })
    });
    (recv.recv().unwrap(), task)
}
fn client(url: String) -> RpcClient {
    RpcClient::connect(ClientOptions::local(
        url,
        "test-secret".into(),
        "workspace".into(),
    ))
}

#[test]
fn calls_wait_for_handshake_and_receive_push_events() {
    let (url, server) = server(false, true);
    let client = client(url);
    let events = client.subscribe("session:event");
    smol::block_on(async {
        let result: Value = client.invoke("echo", vec![json!("hello")]).await.unwrap();
        assert_eq!(result, "hello");
        let event = events.recv().await.unwrap();
        assert_eq!(event[0]["type"], "complete");
        let result: Value = client.invoke("echo", vec![Value::Null]).await.unwrap();
        assert!(result.is_null());
    });
    client.close();
    server.join().unwrap();
}
#[test]
fn disconnected_requests_finish_immediately() {
    let (url, server) = server(true, false);
    let client = client(url);
    let result = smol::block_on(client.invoke::<Value>("echo", vec![json!(1)]));
    assert!(matches!(result, Err(RpcError::Disconnected)));
    client.close();
    server.join().unwrap();
}
#[test]
fn missing_channels_and_utf8_frame_limits_fail_locally() {
    let (url, server) = server(false, true);
    let client = client(url);
    smol::block_on(async {
        assert!(matches!(
            client.invoke::<Value>("missing", vec![]).await,
            Err(RpcError::ChannelUnavailable(_))
        ));
        assert!(matches!(
            client
                .invoke::<Value>("echo", vec![json!("界".repeat(400))])
                .await,
            Err(RpcError::TooLarge)
        ));
    });
    client.close();
    server.join().unwrap();
}
#[test]
fn timeout_does_not_wait_for_a_response_forever() {
    let (url, server) = server(false, false);
    let client = client(url);
    let result =
        smol::block_on(client.invoke_timeout::<Value>("echo", vec![], Duration::from_millis(30)));
    assert!(matches!(result, Err(RpcError::Timeout)));
    client.close();
    server.join().unwrap();
}

fn handshake_server(
    reject: Option<&'static str>,
    stall_after_ack: bool,
) -> (
    String,
    std::sync::mpsc::Sender<()>,
    std::thread::JoinHandle<()>,
) {
    let (port_send, port_recv) = std::sync::mpsc::channel();
    let (stop, wait) = std::sync::mpsc::channel();
    let task = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            port_send
                .send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(stream).await.unwrap();
            let _ = ws.next().await;
            if let Some(code) = reject {
                ws.send(async_tungstenite::tungstenite::Message::Text(
                    json!({"id":"h","type":"error","error":{"code":code,"message":"Rejected"}})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
            } else if stall_after_ack {
                ws.send(async_tungstenite::tungstenite::Message::Text(json!({"id":"h","type":"handshake_ack","clientId":"c","registeredChannels":["echo"],"maxMessageBytes":67108864}).to_string().into())).await.unwrap();
            }
            let _ = wait.recv_timeout(Duration::from_secs(5));
        })
    });
    (port_recv.recv().unwrap(), stop, task)
}
#[test]
fn handshake_authentication_error_reaches_caller() {
    let (url, stop, server) = handshake_server(Some("AUTH_FAILED"), false);
    let client = client(url);
    let result = smol::block_on(smol::future::race(
        client.invoke::<Value>("echo", vec![]),
        async {
            smol::Timer::after(Duration::from_millis(500)).await;
            Err(RpcError::Timeout)
        },
    ));
    let _ = stop.send(());
    server.join().unwrap();
    assert!(matches!(result,Err(RpcError::Wire(ref e)) if e.code.as_str()=="AUTH_FAILED"));
    client.close();
}
#[test]
fn request_timeout_applies_while_handshake_is_stalled() {
    let (url, stop, server) = handshake_server(None, false);
    let client = client(url);
    let result = smol::block_on(smol::future::race(
        async {
            Some(
                client
                    .invoke_timeout::<Value>("echo", vec![], Duration::from_millis(30))
                    .await,
            )
        },
        async {
            smol::Timer::after(Duration::from_millis(300)).await;
            None
        },
    ));
    client.close();
    let _ = stop.send(());
    server.join().unwrap();
    assert!(matches!(result, Some(Err(RpcError::Timeout))));
}
#[test]
fn large_request_obeys_deadline_and_explicit_close_finishes() {
    use agentkit_client::ConnectionState;
    let (url, stop, server) = handshake_server(None, true);
    let client = client(url);
    let result = smol::block_on(smol::future::race(
        async {
            Some(
                client
                    .invoke_timeout::<Value>(
                        "echo",
                        vec![json!("x".repeat(8 * 1024 * 1024))],
                        Duration::from_secs(4),
                    )
                    .await,
            )
        },
        async {
            smol::Timer::after(Duration::from_secs(8)).await;
            None
        },
    ));
    smol::block_on(async {
        smol::Timer::after(Duration::from_millis(200)).await;
    });
    // A host may buffer the whole request, in which case a response timeout
    // legitimately leaves the socket connected. Explicit close must finish in
    // both the buffered-response and blocked-write paths.
    client.close();
    smol::block_on(async {
        let limit = std::time::Instant::now() + Duration::from_secs(2);
        while client.state() != ConnectionState::Disconnected && std::time::Instant::now() < limit {
            smol::Timer::after(Duration::from_millis(10)).await;
        }
    });
    let state = client.state();
    let _ = stop.send(());
    server.join().unwrap();
    assert!(matches!(result, Some(Err(RpcError::Timeout))));
    assert_eq!(state, ConnectionState::Disconnected);
}

#[test]
fn handshake_protocol_error_is_preserved() {
    let (url, stop, server) = handshake_server(Some("PROTOCOL_VERSION_UNSUPPORTED"), false);
    let client = client(url);
    let result =
        smol::block_on(client.invoke_timeout::<Value>("echo", vec![], Duration::from_secs(1)));
    client.close();
    let _ = stop.send(());
    server.join().unwrap();
    assert!(
        matches!(result,Err(RpcError::Wire(ref e)) if e.code.as_str()=="PROTOCOL_VERSION_UNSUPPORTED")
    );
}
#[test]
fn connection_timeout_is_observable_after_stalled_handshake() {
    let (url, stop, server) = handshake_server(None, false);
    let mut options = ClientOptions::local(url, "test-secret".into(), "workspace".into());
    options.connect_timeout = Duration::from_millis(30);
    let client = RpcClient::connect(options);
    let result =
        smol::block_on(client.invoke_timeout::<Value>("echo", vec![], Duration::from_secs(1)));
    client.close();
    let _ = stop.send(());
    server.join().unwrap();
    assert!(matches!(result, Err(RpcError::Timeout)));
    assert!(matches!(client.last_error(), Some(RpcError::Timeout)));
}

#[test]
fn sequence_ack_has_an_id_and_connection_answers_ping() {
    let (port_send, port_recv) = std::sync::mpsc::channel();
    let (ready_send, ready_recv) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            port_send
                .send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = async_tungstenite::accept_async(stream).await.unwrap();
            let _ = socket.next().await;
            socket.send(async_tungstenite::tungstenite::Message::Text(json!({"id":"h","type":"handshake_ack","clientId":"c","registeredChannels":["echo"]}).to_string().into())).await.unwrap();
            socket
                .send(async_tungstenite::tungstenite::Message::Ping(
                    vec![1, 2, 3].into(),
                ))
                .await
                .unwrap();
            let mut pong = false;
            while let Some(Ok(frame)) = socket.next().await {
                if let async_tungstenite::tungstenite::Message::Pong(bytes) = &frame {
                    assert_eq!(bytes.as_ref(), &[1, 2, 3]);
                    pong = true;
                }
                if !frame.is_text() {
                    continue;
                }
                let value: Value = serde_json::from_str(frame.to_text().unwrap()).unwrap();
                if value["type"] == "sequence_ack" {
                    assert!(
                        value["id"].as_str().is_some_and(|id| !id.is_empty()),
                        "Upstream rejects empty IDs even on sequence_ack"
                    );
                    assert!(pong);
                    ready_send.send(()).unwrap();
                }
                if value["type"] == "request" {
                    socket
                        .send(async_tungstenite::tungstenite::Message::Text(
                            json!({"id":value["id"],"type":"response","result":"alive"})
                                .to_string()
                                .into(),
                        ))
                        .await
                        .unwrap();
                    break;
                }
            }
        })
    });
    let client = client(port_recv.recv().unwrap());
    let ready = ready_recv.recv_timeout(Duration::from_secs(8));
    if ready.is_ok() {
        assert_eq!(
            smol::block_on(client.invoke::<String>("echo", vec![])).unwrap(),
            "alive"
        );
    }
    client.close();
    server.join().unwrap();
    assert!(ready.is_ok());
}
