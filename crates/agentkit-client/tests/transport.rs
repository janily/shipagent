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
