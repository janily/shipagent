#![allow(clippy::unwrap_used)]
use super::*;
use futures::StreamExt;
use serde_json::{Value, json};
use std::prelude::v1::test;
struct Mock {
    api: WorkbenchApi,
    requests: std::sync::mpsc::Receiver<Value>,
    responses: async_channel::Sender<Value>,
    thread: std::thread::JoinHandle<()>,
}
fn mock() -> Mock {
    let (endpoint, endpoint_rx) = std::sync::mpsc::channel();
    let (requests, request_rx) = std::sync::mpsc::channel();
    let (responses, response_rx) = async_channel::unbounded::<Value>();
    let thread = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            endpoint
                .send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = async_tungstenite::accept_async(stream).await.unwrap();
            let _hello = socket.next().await.unwrap().unwrap();
            socket.send(async_tungstenite::tungstenite::Message::Text(json!({"type":"handshake_ack","clientId":"draft-test","registeredChannels":agentkit_protocol::constants::channels::ALL}).to_string().into())).await.unwrap();
            while let Some(Ok(frame)) = socket.next().await {
                if frame.is_close() {
                    break;
                }
                let request: Value = serde_json::from_str(frame.to_text().unwrap()).unwrap();
                requests.send(request.clone()).unwrap();
                let Ok(mut response) = response_rx.recv().await else {
                    break;
                };
                response["id"] = request["id"].clone();
                response["type"] = json!("response");
                socket
                    .send(async_tungstenite::tungstenite::Message::Text(
                        response.to_string().into(),
                    ))
                    .await
                    .unwrap();
            }
        })
    });
    let api = WorkbenchApi::connect(
        agentkit_client::Endpoint {
            url: endpoint_rx.recv().unwrap(),
            token: "fixture".into(),
        },
        "w".into(),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while api.bind_connection().is_err() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    Mock {
        api,
        requests: request_rx,
        responses,
        thread,
    }
}
impl Mock {
    fn request(&self) -> Value {
        self.requests.recv_timeout(Duration::from_secs(5)).unwrap()
    }
    fn ok(&self) {
        self.responses.try_send(json!({"result":null})).unwrap();
    }
    fn close(self) {
        self.api.close();
        drop(self.responses);
        self.thread.join().unwrap();
    }
}
fn text(value: &str) -> Snapshot {
    Snapshot {
        text: value.into(),
        ..Default::default()
    }
}
#[test]
fn debounce_coalesces_edits_and_flush_forces_only_latest_snapshot() {
    let backend = mock();
    let writer = Writer::new();
    writer.bind(backend.api.clone());
    writer.save("s".into(), text("old"), false);
    std::thread::sleep(Duration::from_millis(25));
    let last = Instant::now();
    writer.save("s".into(), text("latest 中文"), false);
    let request = backend.request();
    assert!(last.elapsed() >= Duration::from_millis(450));
    assert_eq!(request["channel"], "drafts:set");
    assert_eq!(request["args"], json!(["s",{"text":"latest 中文"}]));
    backend.ok();
    smol::block_on(writer.flush()).unwrap();
    assert!(writer.pending_sequence("s").is_none());
    backend.close();
}
#[test]
fn late_save_and_delete_cannot_overwrite_newer_text() {
    let backend = mock();
    let writer = Writer::new();
    writer.bind(backend.api.clone());
    writer.save("s".into(), text("submitted"), true);
    let request = backend.request();
    assert_eq!(request["channel"], "drafts:set");
    writer.save("s".into(), text(""), true);
    backend.ok();
    let request = backend.request();
    assert_eq!(request["channel"], "drafts:delete");
    assert_eq!(request["args"], json!(["s"]));
    let newest = writer.save("s".into(), text("newer edit"), true);
    backend.ok();
    let request = backend.request();
    assert_eq!(request["channel"], "drafts:set");
    assert_eq!(request["args"][1]["text"], "newer edit");
    backend.ok();
    smol::block_on(writer.flush()).unwrap();
    let mut last = 0;
    while let Ok(n) = writer.notices.try_recv() {
        last = n.sequence;
        assert!(n.result.is_ok());
    }
    assert_eq!(last, newest);
    backend.close();
}
#[test]
fn offline_edits_survive_rebind_and_failed_write_requires_retry() {
    let backend = mock();
    let writer = Writer::new();
    writer.save("s".into(), text("offline old"), false);
    writer.save("s".into(), text("offline latest"), false);
    assert!(smol::block_on(writer.flush()).is_err());
    writer.bind(backend.api.clone());
    let request = backend.request();
    assert_eq!(request["args"][1]["text"], "offline latest");
    backend
        .responses
        .try_send(json!({"error":{"code":"HANDLER_ERROR","message":"fixture disk failure"}}))
        .unwrap();
    let error = smol::block_on(writer.notices.recv()).unwrap();
    assert!(error.result.is_err());
    assert!(writer.pending_sequence("s").is_some());
    assert!(
        backend
            .requests
            .recv_timeout(Duration::from_millis(50))
            .is_err()
    );
    writer.retry("s");
    let request = backend.request();
    assert_eq!(request["args"][1]["text"], "offline latest");
    backend.ok();
    smol::block_on(writer.flush()).unwrap();
    backend.close();
}
