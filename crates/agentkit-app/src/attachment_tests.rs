#![allow(clippy::unwrap_used)]
use std::prelude::v1::test;

use super::*;
use futures::StreamExt;
use serde_json::{Value, json};
#[test]
fn partial_storage_keeps_order_and_sends_converted_paths_and_bytes() {
    let (tx, rx) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        smol::block_on(async {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            tx.send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let hello: Value =
                serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
            ws.send(async_tungstenite::tungstenite::Message::Text(json!({"id":hello["id"],"type":"handshake_ack","clientId":"attachments","maxMessageBytes":4096,"registeredChannels":agentkit_protocol::constants::channels::ALL}).to_string().into())).await.unwrap();
            for index in 0..3 {
                let req: Value =
                    serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap())
                        .unwrap();
                assert_eq!(req["channel"], "file:storeAttachment");
                assert_eq!(req["args"][0], "original-session");
                assert_eq!(req["args"][1]["name"], format!("file-{index}.png"));
                let response = if index == 1 {
                    json!({"id":req["id"],"type":"response","error":{"code":"INTERNAL_ERROR","message":"fixture storage failure"}})
                } else {
                    json!({"id":req["id"],"type":"response","result":{"id":format!("stored-{index}"),"type":"image","name":format!("file-{index}.png"),"mimeType":"image/jpeg","size":2,"storedPath":format!("/stored/{index}.jpg"),"markdownPath":format!("/stored/{index}.md"),"resizedBase64":"RESIZED"}})
                };
                ws.send(async_tungstenite::tungstenite::Message::Text(
                    response.to_string().into(),
                ))
                .await
                .unwrap();
            }
            let req: Value =
                serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
            assert_eq!(req["channel"], "sessions:sendMessage");
            let args = &req["args"];
            assert_eq!(args[0], "original-session");
            assert_eq!(args[1], "");
            assert_eq!(args[2].as_array().unwrap().len(), 2);
            for (i, index) in [0, 2].into_iter().enumerate() {
                assert_eq!(args[2][i]["storedPath"], format!("/stored/{index}.jpg"));
                assert_eq!(args[2][i]["markdownPath"], format!("/stored/{index}.md"));
                assert_eq!(args[2][i]["mimeType"], "image/jpeg");
                assert_eq!(args[2][i]["base64"], "RESIZED");
                assert_eq!(args[3][i]["id"], format!("stored-{index}"));
            }
            assert_eq!(args[4]["optimisticMessageId"], "attachment-send");
            ws.send(async_tungstenite::tungstenite::Message::Text(json!({"id":req["id"],"type":"response","result":{"accepted":true,"messageId":"attachment-send"}}).to_string().into())).await.unwrap();
            while ws.next().await.is_some() {}
        })
    });
    smol::block_on(async {
        let api = WorkbenchApi::connect(
            agentkit_client::Endpoint {
                url: rx.recv().unwrap(),
                token: "fixture".into(),
            },
            "workspace".into(),
        );
        let mut entries = (0..3)
            .map(|i| Entry {
                id: format!("local-{i}"),
                file: std::sync::Arc::new(g::FileAttachment {
                    path: format!("/original/{i}.png"),
                    name: format!("file-{i}.png"),
                    mime_type: "image/png".into(),
                    type_: g::FileAttachmentType::Image,
                    size: 3,
                    base64: Some("ORIGINAL".into()),
                    thumbnail_base64: None,
                    text: None,
                }),
            })
            .collect::<Vec<_>>();
        let mut oversized = (*entries[0].file).clone();
        oversized.name = "over-limit.png".into();
        oversized.size = 8192;
        oversized.base64 = Some("A".repeat(8192));
        entries.push(Entry {
            id: "over-limit".into(),
            file: std::sync::Arc::new(oversized),
        });
        let batch = store_batch(&api, "original-session", &entries).await;
        assert_eq!(batch.ids, ["local-0", "local-2"]);
        assert_eq!(batch.failures.len(), 2);
        assert!(
            api.send_prepared(
                "original-session",
                "",
                "attachment-send",
                &batch.files,
                &batch.stored
            )
            .await
            .unwrap()
            .accepted
        );
        api.close();
    });
    server.join().unwrap();
}

struct Model(std::process::Child);
impl Drop for Model {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[test]
#[ignore = "Requires bundled backend and Node; isolated local model"]
fn real_attachment_storage_send_and_restart() {
    use agentkit_sidecar::{SidecarConfig, SidecarHandle};
    use std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    smol::block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let backend = std::path::PathBuf::from(std::env::var_os("AGENTKIT_BACKEND_DIR").unwrap());
        let config = temp.path().join("config");
        let sidecar = SidecarHandle::start(SidecarConfig::new(backend.clone(), config.clone()))
            .await
            .unwrap();
        let initial = WorkbenchApi::connect(sidecar.endpoint(), String::new());
        let workspace = initial.workspaces_generated().await.unwrap().remove(0);
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut model = Model(
            Command::new("node")
                .arg(repo.join("tools/fixtures/model.mjs"))
                .arg(repo.join("tools/fake-llm/scenarios/text.yaml"))
                .env("AGENTKIT_FIXTURE_ROOT", temp.path())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap(),
        );
        let stdout = model.0.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            BufReader::new(stdout).read_line(&mut line).unwrap();
            tx.send(line).unwrap();
        });
        let line = rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let url = line.trim().strip_prefix("FAKE_LLM_URL=").unwrap();
        initial
            .setup_endpoint(url, "fixture-model", None)
            .await
            .unwrap();
        initial.disable_browser().await.unwrap();
        initial.close();
        let api = WorkbenchApi::connect(sidecar.endpoint(), workspace.id.clone());
        let s = api
            .create_session(&workspace.id, Some(&workspace.root_path))
            .await
            .unwrap();
        let good = temp.path().join("context.txt");
        std::fs::write(&good, "Actual attachment 中文").unwrap();
        let entries = vec![
            Entry {
                id: "good".into(),
                file: std::sync::Arc::new(crate::files::read(&good).unwrap()),
            },
            Entry {
                id: "empty".into(),
                file: std::sync::Arc::new(
                    crate::files::from_bytes("/empty".into(), "empty.txt".into(), Vec::new())
                        .unwrap(),
                ),
            },
        ];
        let batch = store_batch(&api, &s.id, &entries).await;
        assert_eq!(batch.ids, ["good"]);
        assert_eq!(batch.failures.len(), 1);
        let path = batch.stored[0].stored_path.clone();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "Actual attachment 中文"
        );
        assert!(
            api.send_prepared(&s.id, "", "real-attachment", &batch.files, &batch.stored)
                .await
                .unwrap()
                .accepted
        );
        let deadline = Instant::now() + Duration::from_secs(40);
        loop {
            let snapshot = api.session_generated(&s.id).await.unwrap();
            if !snapshot.is_processing
                && snapshot
                    .messages
                    .iter()
                    .any(|m| m.role == g::MessageRole::Assistant)
            {
                assert!(
                    snapshot
                        .messages
                        .iter()
                        .any(|m| m.role == g::MessageRole::User
                            && m.attachments.as_ref().is_some_and(|a| a.len() == 1))
                );
                break;
            }
            assert!(
                Instant::now() < deadline,
                "attachment turn did not finish: {:?}",
                snapshot.messages
            );
            smol::Timer::after(Duration::from_millis(30)).await;
        }
        api.close();
        sidecar.shutdown(Duration::from_secs(5)).await.unwrap();
        let restarted = SidecarHandle::start(SidecarConfig::new(backend, config))
            .await
            .unwrap();
        let api = WorkbenchApi::connect(restarted.endpoint(), workspace.id);
        let snapshot = api.session_generated(&s.id).await.unwrap();
        assert!(snapshot.messages.iter().any(|m| {
            m.role == g::MessageRole::User
                && m.attachments
                    .as_ref()
                    .is_some_and(|a| a.iter().any(|a| a.stored_path == path))
        }));
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "Actual attachment 中文"
        );
        api.close();
        restarted.shutdown(Duration::from_secs(5)).await.unwrap();
    });
}
