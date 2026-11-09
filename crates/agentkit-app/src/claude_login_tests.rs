#![allow(clippy::unwrap_used)]
use super::*;
use crate::controller::oauth_test_support as support;
use serde_json::json;
use support::*;
#[test]
fn exchange_save_identity_verify_and_failure_cleanup_are_serialized() {
    for mode in 0..6 {
        run(mode);
    }
    active_login_rejects_a_second_window_without_clearing_the_first();
}
#[cfg(test)]
fn run(mode: u8) {
    let (cancel, cancelled) = async_channel::bounded(1);
    let server_cancel = cancel.clone();
    let (codes_tx, codes) = async_channel::bounded(1);
    let (ready_tx, ready) = async_channel::bounded(1);
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
            let mut setup = false;
            let mut tested = false;
            let mut deleted = false;
            let mut wiped = false;
            loop {
                let req = frame(&mut ws).await;
                let channel = req["channel"].as_str().unwrap();
                let value = match channel {
                    "onboarding:startClaudeOAuth" => {
                        json!({"success":true,"authUrl":format!("https://claude.ai/oauth/authorize?state=fixture&code_challenge=challenge&code_challenge_method=S256&redirect_uri=https%3A%2F%2Fconsole.anthropic.com%2Foauth%2Fcode%2Fcallback{}",if mode==4{"&redirect_uri=https%3A%2F%2Fevil.example"}else{""})})
                    }
                    "onboarding:exchangeClaudeCode" => {
                        assert_eq!(req["args"][0], "fixture-code");
                        assert_eq!(req["args"][1], "claude-max-123");
                        if mode == 1 {
                            json!({"success":false,"error":"sensitive-provider-response"})
                        } else {
                            json!({"success":true,"token":"fixture-token","identity":{"account":{"uuid":"fixture-account","emailAddress":"fixture@example.invalid"}}})
                        }
                    }
                    "settings:setupLlmConnection" => {
                        assert_eq!(req["args"][0]["credential"], "fixture-token");
                        assert_eq!(
                            req["args"][0]["oauthIdentity"]["account"]["uuid"],
                            "fixture-account"
                        );
                        setup = true;
                        if mode == 2 {
                            server_cancel.close();
                        }
                        json!({"success":mode!=2})
                    }
                    "LLM_Connection:test" => {
                        assert!(setup);
                        tested = true;
                        json!({"success":true})
                    }
                    "LLM_Connection:setDefault" => {
                        assert!(tested);
                        json!({"success":true})
                    }
                    "LLM_Connection:listWithStatus" => {
                        assert!(tested);
                        json!([{"slug":"claude-max-123","name":"Claude","providerType":"pi","authType":"oauth","createdAt":1,"isAuthenticated":true,"isDefault":true}])
                    }
                    "LLM_Connection:delete" => {
                        deleted = true;
                        json!({"success":true})
                    }
                    "chatgpt:logout" => {
                        wiped = true;
                        json!({"success":true})
                    }
                    "onboarding:clearClaudeOAuthState" => {
                        if mode != 0 && mode != 5 {
                            assert!(deleted && wiped);
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
                if (channel == "onboarding:clearClaudeOAuthState" && mode != 5)
                    || (channel == "chatgpt:logout" && mode == 5)
                {
                    break;
                }
            }
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
        let sender = smol::spawn(async move {
            if ready.recv().await.is_ok() {
                if mode == 3 {
                    drop(cancel);
                } else {
                    codes_tx.send("fixture-code".into()).await.unwrap();
                    drop(codes_tx);
                    let _keep = cancel;
                    smol::future::pending::<()>().await;
                }
            }
        });
        let result = login(
            Request {
                api,
                slug: "claude-max-123".into(),
                onboarding: true,
                cancel: cancelled,
                code: codes,
                ready: ready_tx,
                lease: Default::default(),
            },
            |_| Ok(()),
        )
        .await;
        match (mode, result) {
            (0, Ok(pending)) => {
                assert_eq!(pending.accept().len(), 1);
            }
            (5, Ok(pending)) => {
                drop(pending);
            }
            (1, Err(LoginError::Code))
            | (2 | 3, Err(LoginError::Cancelled))
            | (4, Err(LoginError::Browser)) => {}
            _ => panic!("unexpected login outcome"),
        }
        sender.cancel().await;
    });
    server.join().unwrap();
}
#[cfg(test)]
fn active_login_rejects_a_second_window_without_clearing_the_first() {
    let active = std::sync::Arc::new(smol::lock::Mutex::new(()));
    let lease = active.try_lock().unwrap();
    smol::block_on(async {
        let api = WorkbenchApi::connect(
            agentkit_client::Endpoint {
                url: "ws://127.0.0.1:1".into(),
                token: "fixture".into(),
            },
            String::new(),
        );
        let (_cancel, rx) = async_channel::bounded(1);
        let (_code, codes) = async_channel::bounded(1);
        let (ready, _) = async_channel::bounded(1);
        assert!(matches!(
            login(
                Request {
                    api,
                    slug: "claude-max-2".into(),
                    onboarding: false,
                    cancel: rx,
                    code: codes,
                    ready,
                    lease: active.clone()
                },
                |_| panic!()
            )
            .await,
            Err(LoginError::Busy)
        ));
    });
    drop(lease);
}

#[test]
fn code_wait_times_out_and_closed_window_cancels() {
    smol::block_on(async {
        let (_tx, code) = async_channel::bounded(1);
        let (cancel, rx) = async_channel::bounded(1);
        assert!(matches!(
            wait_code(code.clone(), rx.clone(), Duration::from_millis(1)).await,
            Err(LoginError::Timeout)
        ));
        drop(cancel);
        assert!(matches!(
            wait_code(code, rx, Duration::from_secs(1)).await,
            Err(LoginError::Cancelled)
        ));
    });
}
