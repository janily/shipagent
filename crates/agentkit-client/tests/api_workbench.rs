#![allow(clippy::unwrap_used)]
mod support;
use agentkit_client::{Endpoint, api::WorkbenchApi};
use serde_json::json;
use support::*;
fn serve(
    results: Vec<(&'static str, serde_json::Value)>,
) -> (Endpoint, std::thread::JoinHandle<()>) {
    serve_checked(results, |_| {})
}
fn serve_checked(
    results: Vec<(&'static str, serde_json::Value)>,
    inspect: impl Fn(&serde_json::Value) + Send + 'static,
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
                inspect(&req);
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

#[test]
fn user_branch_sends_canonical_source_and_safe_permission_mode_once() {
    let snapshot = json!({"id":"branch","workspaceId":"w","workspaceName":"W","messages":[],"lastMessageAt":0,"isProcessing":false});
    let (endpoint, server) = serve_checked(vec![("sessions:create", snapshot)], |request| {
        assert_eq!(request["args"][0], "w");
        let options = &request["args"][1];
        assert_eq!(options["branchFromSessionId"], "source");
        assert_eq!(options["branchFromMessageId"], "user");
        assert_eq!(options["permissionMode"], "safe");
        assert_eq!(options["workingDirectory"], "/project");
    });
    let api = WorkbenchApi::connect(endpoint, "w".into());
    let session =
        smol::block_on(api.branch_session_generated("w", "source", "user", Some("/project")))
            .unwrap();
    assert_eq!(session.id, "branch");
    api.close();
    server.join().unwrap();
}

#[test]
fn composer_configuration_is_session_scoped_and_preserves_model_connection_pair() {
    let (endpoint, server) = serve_checked(
        vec![
            ("session:setModel", json!(null)),
            ("sessions:command", json!(null)),
            ("sessions:command", json!(null)),
        ],
        |request| {
            if request["channel"] == "session:setModel" {
                assert_eq!(
                    request["args"],
                    json!(["s", "w", "different-model", "authenticated-connection"])
                );
            } else {
                assert_eq!(request["args"][0], "s");
                let command = &request["args"][1];
                match command["type"].as_str().unwrap() {
                    "setThinkingLevel" => assert_eq!(command["level"], "high"),
                    "setPermissionMode" => assert_eq!(command["mode"], "ask"),
                    other => panic!("unexpected command: {other}"),
                }
            }
        },
    );
    let api = WorkbenchApi::connect(endpoint, "w".into());
    smol::block_on(async {
        api.set_session_model("s", "w", "different-model", "authenticated-connection")
            .await
            .unwrap();
        api.set_session_thinking("s", agentkit_protocol::generated::ThinkingLevel::High)
            .await
            .unwrap();
        api.set_session_mode("s", agentkit_protocol::generated::PermissionMode::Ask)
            .await
            .unwrap();
    });
    api.close();
    server.join().unwrap();
}

#[test]
fn mention_options_and_search_preserve_exact_positional_contracts() {
    use agentkit_protocol::generated as g;
    let options = json!({"badges":[{"type":"file","label":"你好.rs","rawText":"[file:src/你好.rs]","filePath":"src/你好.rs","start":3.0,"end":19.0}],"skillSlugs":["review-code"]});
    let expected = options.clone();
    let (endpoint, server) = serve_checked(
        vec![
            ("fs:search", json!([])),
            (
                "sessions:sendMessage",
                json!({"accepted":true,"messageId":"optimistic"}),
            ),
            (
                "sessions:sendMessage",
                json!({"accepted":true,"messageId":"optimistic"}),
            ),
        ],
        move |req| {
            if req["channel"] == "fs:search" {
                assert_eq!(req["args"], json!(["/session/directory", "src"]));
            } else {
                assert_eq!(req["args"].as_array().unwrap().len(), 5);
                assert_eq!(req["args"][0], "s");
                assert_eq!(req["args"][1], "😀 [file:src/你好.rs]");
                assert_eq!(req["args"][2], json!([]));
                assert_eq!(req["args"][3], json!([]));
                let mut options = expected.clone();
                options["optimisticMessageId"] = json!("optimistic");
                assert_eq!(req["args"][4], options);
            }
        },
    );
    let api = WorkbenchApi::connect(endpoint, "w".into());
    smol::block_on(async {
        api.search_mentions("/session/directory", "src")
            .await
            .unwrap();
        let options: g::SendMessageOptions = serde_json::from_value(options).unwrap();
        assert!(
            api.send_with_attachments_with_options(
                "s",
                "😀 [file:src/你好.rs]",
                "optimistic",
                &[],
                &[],
                options.clone()
            )
            .await
            .unwrap()
            .accepted
        );
        assert!(
            api.send_prepared_with_options(
                "s",
                "😀 [file:src/你好.rs]",
                "optimistic",
                &[],
                &[],
                options
            )
            .await
            .unwrap()
            .accepted
        );
    });
    api.close();
    server.join().unwrap();
}
