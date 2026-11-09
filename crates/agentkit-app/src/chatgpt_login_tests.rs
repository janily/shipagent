#![allow(clippy::unwrap_used)]
use super::login;
#[path = "../../agentkit-client/tests/support/mod.rs"]
mod support;
use agentkit_client::{Endpoint, api::WorkbenchApi};
use serde_json::json;
use smol::io::{AsyncReadExt, AsyncWriteExt};
use support::*;
#[test]
fn browser_callback_completes_typed_flow_and_releases_port_with_no_native_tokens() {
    for mode in 0..3 {
        run(mode);
    }
}
#[cfg(test)]
fn run(mode: u8) {
    let (cancel, cancelled) = async_channel::bounded(1);
    let server_cancel = cancel.clone();
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
            let mut complete = false;
            let mut setup = false;
            let mut deleted = false;
            let mut logged_out = false;
            loop {
                let req = frame(&mut ws).await;
                let channel = req["channel"].as_str().unwrap();
                let value = match channel {
                    "chatgpt:startOAuth" => {
                        json!({"state":"fixture-state","flowId":"fixture-flow","authUrl":format!("https://auth.openai.com/oauth/authorize?state=fixture-state&redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback{}",if mode==2{"&redirect_uri=https%3A%2F%2Fevil.example"}else{""})})
                    }
                    "chatgpt:completeOAuth" => {
                        assert_eq!(req["args"][0]["state"], "fixture-state");
                        assert_eq!(req["args"][0]["code"], "fixture-code");
                        assert_eq!(req["args"][0]["flowId"], "fixture-flow");
                        complete = true;
                        json!({"success":true})
                    }
                    "settings:setupLlmConnection" => {
                        assert!(complete);
                        assert_eq!(req["args"][0]["slug"], "chatgpt-plus-123");
                        setup = true;
                        json!({"success":true})
                    }
                    "chatgpt:getAuthStatus" => {
                        assert!(setup);
                        if mode == 1 {
                            server_cancel.close();
                        }
                        json!({"authenticated":mode!=1})
                    }
                    "LLM_Connection:delete" => {
                        deleted = true;
                        json!({"success":true})
                    }
                    "chatgpt:logout" => {
                        logged_out = true;
                        json!({"success":true})
                    }
                    "chatgpt:cancelOAuth" => {
                        if mode == 1 {
                            assert!(deleted && logged_out);
                        }
                        if mode == 2 {
                            assert!(!complete && !setup);
                        }
                        json!({"success":true})
                    }
                    _ => panic!("unexpected channel"),
                };
                send(
                    &mut ws,
                    json!({"id":req["id"],"type":"response","channel":channel,"result":value}),
                )
                .await;
                if channel == "chatgpt:cancelOAuth" {
                    break;
                }
            }
        })
    });
    smol::block_on(async {
        let api = WorkbenchApi::connect(
            Endpoint {
                url: rx.recv().unwrap(),
                token: "fixture-token".into(),
            },
            "workspace".into(),
        );
        let rx = cancelled;
        let result=login(api,"chatgpt-plus-123",rx,"Response received".into(),|url|{
            assert!(url.starts_with("https://auth.openai.com/"));
            smol::spawn(async {
                let mut stream=async_net::TcpStream::connect("127.0.0.1:1455").await.unwrap();
                stream.write_all(b"GET /auth/callback?state=fixture-state&code=fixture-code HTTP/1.1\r\nHost: localhost:1455\r\n\r\n").await.unwrap();
                let mut page=String::new();stream.read_to_string(&mut page).await.unwrap();assert!(page.contains("200 OK"));assert!(!page.contains("fixture-code"));
            }).detach();Ok(())
        }).await;
        match mode {
            0 => assert!(result.is_ok()),
            1 => assert!(matches!(
                result,
                Err(agentkit_capabilities::oauth_chatgpt::LoginError::Cancelled)
            )),
            _ => assert!(matches!(
                result,
                Err(agentkit_capabilities::oauth_chatgpt::LoginError::Browser)
            )),
        }
        assert!(async_net::TcpListener::bind("127.0.0.1:1455").await.is_ok());
    });
    server.join().unwrap();
}
