#![allow(clippy::unwrap_used)]
mod support;
use agentkit_client::{ClientOptions, RpcClient};
use agentkit_protocol::{ErrorCode, RpcValue, WireError};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use support::*;

#[test]
fn slow_capability_does_not_block_events_or_responses_and_binary_leaves_round_trip() {
    let (urls, recv) = std::sync::mpsc::channel();
    let (release, wait) = async_channel::bounded(1);
    let server = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            urls.send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let h = frame(&mut ws).await;
            assert_eq!(h["clientCapabilities"], json!(["client:slow"]));
            send(&mut ws,json!({"type":"handshake_ack","clientId":"client","registeredChannels":["echo"],"maxMessageBytes":1024})).await;
            send(&mut ws,json!({"type":"request","id":"reverse","channel":"client:slow","args":[{"__craftRpcType":"u8","base64":"AQID"}]})).await;
            let req = frame(&mut ws).await;
            assert_eq!(req["channel"], "echo");
            assert_eq!(req["args"][0]["__craftRpcType"], "u8");
            send(
                &mut ws,
                json!({"type":"response","id":req["id"],"result":req["args"][0]}),
            )
            .await;
            send(&mut ws,json!({"type":"event","channel":"bytes","seq":1,"args":[{"__craftRpcType":"u8","base64":"BAU="}]})).await;
            let reply = frame(&mut ws).await;
            assert_eq!(reply["id"], "reverse");
            assert_eq!(reply["result"]["base64"], "Bgc=");
        })
    });
    let mut options =
        ClientOptions::local(recv.recv().unwrap(), "secret".into(), "workspace".into());
    options.capabilities.insert(
        "client:slow".into(),
        Arc::new(move |args| {
            let wait = wait.clone();
            Box::pin(async move {
                assert_eq!(args[0], RpcValue::Bytes(vec![1, 2, 3]));
                wait.recv().await.unwrap();
                Ok(RpcValue::Bytes(vec![6, 7]))
            })
        }),
    );
    options.capabilities.insert(
        "client:browser:invoke".into(),
        Arc::new(|_| Box::pin(async { Ok(RpcValue::Null) })),
    );
    let client = RpcClient::connect(options);
    let events = client.subscribe_values("bytes");
    smol::block_on(async {
        let reply = client
            .invoke_values("echo", vec![RpcValue::Bytes(vec![8, 9])])
            .await
            .unwrap();
        assert_eq!(reply, RpcValue::Bytes(vec![8, 9]));
        assert_eq!(
            bounded(events.recv()).await.unwrap().unwrap().unwrap()[0],
            RpcValue::Bytes(vec![4, 5])
        );
        release.send(()).await.unwrap();
    });
    server.join().unwrap();
    client.close();
}

#[test]
fn capability_errors_timeouts_panics_and_oversized_replies_are_correlated() {
    let (urls, recv) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            urls.send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            send(&mut ws,json!({"type":"handshake_ack","clientId":"client","registeredChannels":["echo"],"maxMessageBytes":512})).await;
            for name in ["unknown", "slow", "panic", "large", "error"] {
                send(&mut ws,json!({"type":"request","id":name,"channel":format!("client:{name}"),"args":[]})).await;
            }
            let mut codes = std::collections::BTreeMap::new();
            for _ in 0..5 {
                let reply = frame(&mut ws).await;
                assert_eq!(
                    reply["type"], "response",
                    "upstream correlates reverse failures only as responses"
                );
                codes.insert(
                    reply["id"].as_str().unwrap().to_owned(),
                    reply["error"]["code"].as_str().unwrap().to_owned(),
                );
            }
            assert_eq!(codes["unknown"], "CAPABILITY_UNAVAILABLE");
            assert_eq!(codes["slow"], "CLIENT_REQUEST_TIMEOUT");
            assert_eq!(codes["panic"], "HANDLER_ERROR");
            assert_eq!(codes["large"], "TRANSFER_TOO_LARGE");
            assert_eq!(codes["error"], "HANDLER_ERROR");
        })
    });
    let mut options =
        ClientOptions::local(recv.recv().unwrap(), "secret".into(), "workspace".into());
    options.capability_timeout = Duration::from_millis(100);
    options.capabilities.insert(
        "client:slow".into(),
        Arc::new(|_| Box::pin(std::future::pending())),
    );
    options.capabilities.insert(
        "client:panic".into(),
        Arc::new(|_| Box::pin(async { panic!("test capability panic") })),
    );
    options.capabilities.insert(
        "client:large".into(),
        Arc::new(|_| Box::pin(async { Ok(RpcValue::String("界".repeat(200))) })),
    );
    options.capabilities.insert(
        "client:error".into(),
        Arc::new(|_| {
            Box::pin(async {
                Err(WireError {
                    code: ErrorCode::HandlerError,
                    message: "denied".into(),
                    data: None,
                })
            })
        }),
    );
    let client = RpcClient::connect(options);
    server.join().unwrap();
    client.close();
}

#[test]
fn outgoing_invalid_binary_is_rejected_before_transmission() {
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
                json!({"type":"handshake_ack","clientId":"client","registeredChannels":["echo"]}),
            )
            .await;
            let good = frame(&mut ws).await;
            assert_eq!(good["args"][0], "valid");
            send(
                &mut ws,
                json!({"type":"response","id":good["id"],"result":true}),
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
        assert!(
            client
                .invoke::<Value>("echo", vec![json!({"__craftRpcType":"u8","base64":"!"})])
                .await
                .is_err()
        );
        assert!(
            client
                .invoke::<bool>("echo", vec![json!("valid")])
                .await
                .unwrap()
        );
    });
    client.close();
    server.join().unwrap();
}

#[test]
fn disconnected_capability_is_cancelled_and_cannot_reply_on_new_socket() {
    use std::sync::atomic::{AtomicBool, Ordering};
    struct Cancelled(Arc<AtomicBool>);
    impl Drop for Cancelled {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let mark = cancelled.clone();
    let (urls, recv) = std::sync::mpsc::channel();
    let (started, starting) = async_channel::bounded(1);
    let (release, wait) = async_channel::bounded::<()>(1);
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
                json!({"type":"handshake_ack","clientId":"client","registeredChannels":["drop"]}),
            )
            .await;
            send(
                &mut ws,
                json!({"type":"request","id":"old-reply","channel":"client:slow","args":[]}),
            )
            .await;
            let _ = frame(&mut ws).await;
            ws.close(None).await.unwrap();
            let (tcp, _) = bounded(listener.accept()).await.unwrap().unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            send(&mut ws,json!({"type":"handshake_ack","clientId":"client","reconnected":true,"registeredChannels":["echo"]})).await;
            let req = frame(&mut ws).await;
            assert_eq!(req["type"], "request");
            assert_eq!(req["channel"], "echo");
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
    options.capabilities.insert(
        "client:slow".into(),
        Arc::new(move |_| {
            let wait = wait.clone();
            let mark = mark.clone();
            let started = started.clone();
            Box::pin(async move {
                let _guard = Cancelled(mark);
                started.send(()).await.unwrap();
                let _ = wait.recv().await;
                Ok(RpcValue::Null)
            })
        }),
    );
    let client = RpcClient::connect(options);
    let recovery = client.subscribe("__transport:reconnected");
    smol::block_on(async {
        bounded(starting.recv()).await.unwrap().unwrap();
        assert!(client.invoke::<Value>("drop", vec![]).await.is_err());
        bounded(recovery.recv()).await.unwrap().unwrap();
        assert!(cancelled.load(Ordering::SeqCst));
        let _ = release.try_send(());
        assert!(client.invoke::<bool>("echo", vec![]).await.unwrap());
    });
    client.close();
    server.join().unwrap();
}

#[test]
fn frame_limit_counts_base64_expansion() {
    let (urls, recv) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            urls.send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            send(&mut ws,json!({"type":"handshake_ack","clientId":"client","registeredChannels":["echo"],"maxMessageBytes":512})).await;
            let req = frame(&mut ws).await;
            assert_eq!(req["args"], json!([]));
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
                .invoke_values("echo", vec![RpcValue::Bytes(vec![0; 400])])
                .await,
            Err(agentkit_client::RpcError::TooLarge)
        ));
        assert!(client.invoke::<bool>("echo", vec![]).await.unwrap());
    });
    client.close();
    server.join().unwrap();
}
