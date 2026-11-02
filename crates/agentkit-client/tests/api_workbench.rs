#![allow(clippy::unwrap_used)]
mod support;
use agentkit_client::{Endpoint, api::WorkbenchApi};
use serde_json::json;
use support::*;
fn serve(
    results: Vec<(&'static str, serde_json::Value)>,
) -> (Endpoint, std::thread::JoinHandle<()>) {
    let (urls, recv) = std::sync::mpsc::channel();
    let task = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            urls.send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            send(&mut ws,json!({"type":"handshake_ack","clientId":"shim","registeredChannels":agentkit_protocol::constants::channels::ALL})).await;
            for (channel, result) in results {
                let req = frame(&mut ws).await;
                assert_eq!(req["channel"], channel);
                send(
                    &mut ws,
                    json!({"type":"response","id":req["id"],"result":result}),
                )
                .await;
            }
        })
    });
    (
        Endpoint {
            url: recv.recv().unwrap(),
            token: "key".into(),
        },
        task,
    )
}
#[test]
fn legacy_ui_bridge_validates_full_wire_session_before_projection() {
    let (endpoint, server) = serve(vec![("sessions:get", json!([{"id":"s"}]))]);
    let api = WorkbenchApi::connect(endpoint, "w".into());
    let result = smol::block_on(api.sessions());
    api.close();
    server.join().unwrap();
    assert!(
        result.is_err(),
        "malformed full DTO became a defaulted UI session"
    );
}
#[test]
fn setup_does_not_report_success_when_default_selection_fails() {
    let (endpoint, server) = serve(vec![
        ("settings:setupLlmConnection", json!({"success":true})),
        (
            "LLM_Connection:setDefault",
            json!({"success":false,"error":"default rejected"}),
        ),
    ]);
    let api = WorkbenchApi::connect(endpoint, "w".into());
    let result = smol::block_on(api.setup_endpoint("http://127.0.0.1:9", "model", None));
    api.close();
    server.join().unwrap();
    assert!(result.is_err(), "default failure was ignored");
    assert_eq!(result.unwrap_err().protocol_code(), "HANDLER_ERROR");
}
#[test]
fn unaccepted_permission_response_is_not_treated_as_success() {
    let (endpoint, server) = serve(vec![("sessions:respondToPermission", json!(false))]);
    let api = WorkbenchApi::connect(endpoint, "w".into());
    let result = smol::block_on(api.respond_permission("s", "r", false, false));
    api.close();
    server.join().unwrap();
    assert!(result.is_err(), "unaccepted response was ignored");
}
