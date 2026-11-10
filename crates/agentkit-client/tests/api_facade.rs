#![allow(clippy::unwrap_used)]
mod support;
use agentkit_client::api::{secrets::*, types::*};
use agentkit_client::{ClientOptions, RpcClient};
use agentkit_protocol::generated as g;
use futures::StreamExt;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use support::*;
struct Expected {
    channel: &'static str,
    args: Value,
    result: Value,
}
#[test]
fn every_v1_request_uses_typed_returns_and_exact_positional_wire_arguments() {
    let (urls, recv) = std::sync::mpsc::channel();
    let (expected, waiting) = async_channel::unbounded::<Expected>();
    let server = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            urls.send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            send(&mut ws,json!({"type":"handshake_ack","clientId":"fixture","registeredChannels":agentkit_protocol::constants::channels::ALL})).await;
            let mut visited = BTreeSet::new();
            while let Ok(case) = waiting.recv().await {
                let req = frame(&mut ws).await;
                assert_eq!(req["channel"], case.channel);
                assert_eq!(req["args"], case.args, "{}", case.channel);
                visited.insert(case.channel);
                send(
                    &mut ws,
                    json!({"type":"response","id":req["id"],"result":case.result}),
                )
                .await;
            }
            visited
        })
    });
    let client = RpcClient::connect(ClientOptions::local(
        recv.recv().unwrap(),
        "fixture-key".into(),
        "w".into(),
    ));
    let session = json!({"id":"s","workspaceId":"w","workspaceName":"W","lastMessageAt":1,"messages":[],"isProcessing":false});
    let workspace = json!({"id":"w","slug":"w","name":"W","rootPath":"/tmp/w","createdAt":1});
    let connection =
        json!({"slug":"test","name":"Test","providerType":"pi","authType":"none","createdAt":1.0});
    let bundle = json!({"version":1,"session":{"header":{"id":"s","workspaceRootPath":"/tmp/w","createdAt":1.0,"lastUsedAt":1.0,"messageCount":0.0,"tokenUsage":{"inputTokens":0.0,"outputTokens":0.0,"totalTokens":0.0,"contextTokens":0.0,"costUsd":0.0}},"messages":[]},"files":[]});
    macro_rules! rpc {
        ($ty:ty,$call:expr,$channel:literal,$args:expr,$result:expr) => {{
            expected
                .send(Expected {
                    channel: $channel,
                    args: $args,
                    result: $result,
                })
                .await
                .unwrap();
            let value: $ty = $call.await.unwrap();
            value
        }};
    }
    smol::block_on(async {
        rpc!(
            g::ServerHealth,
            client.service().health(),
            "server:getHealth",
            json!([]),
            json!({"status":"ok","checks":[]})
        );
        rpc!(
            Versions,
            client.service().versions(),
            "system:versions",
            json!([]),
            json!({"node":"24"})
        );
        rpc!(
            Vec<g::Workspace>,
            client.workspaces().list(),
            "workspaces:get",
            json!([]),
            json!([workspace.clone()])
        );
        rpc!(
            g::Workspace,
            client.workspaces().create("/tmp/w", "W"),
            "workspaces:create",
            json!(["/tmp/w", "W"]),
            workspace.clone()
        );
        rpc!(
            SlugCheck,
            client.workspaces().check_slug("w"),
            "workspaces:checkSlug",
            json!(["w"]),
            json!({"exists":true,"path":"/tmp/w"})
        );
        rpc!(
            bool,
            client.workspaces().remove("w"),
            "workspaces:remove",
            json!(["w"]),
            json!(false)
        );
        rpc!(
            (),
            client.workspaces().switch("w"),
            "window:switchWorkspace",
            json!(["w"]),
            json!({"workspaceId":"w"})
        );
        rpc!(
            Option<g::WorkspaceSettings>,
            client.workspaces().settings("w"),
            "workspaceSettings:get",
            json!(["w"]),
            json!({"codingMode":false})
        );
        rpc!(
            (),
            client
                .workspaces()
                .update("w", &WorkspaceSetting::CodingMode(true)),
            "workspaceSettings:update",
            json!(["w", "codingMode", true]),
            Value::Null
        );
        rpc!(
            Option<PermissionsConfig>,
            client.workspaces().permissions("w"),
            "workspace:getPermissions",
            json!(["w"]),
            Value::Null
        );
        rpc!(
            Vec<g::Session>,
            client.sessions().list(),
            "sessions:get",
            json!([]),
            json!([session.clone()])
        );
        let missing = rpc!(
            Option<g::Session>,
            client.sessions().get_messages("missing"),
            "sessions:getMessages",
            json!(["missing"]),
            Value::Null
        );
        assert!(missing.is_none());
        rpc!(
            g::Session,
            client.sessions().create("w", None),
            "sessions:create",
            json!(["w"]),
            session.clone()
        );
        rpc!(
            (),
            client.sessions().delete("s"),
            "sessions:delete",
            json!(["s"]),
            Value::Null
        );
        let options: g::SendMessageOptions =
            serde_json::from_value(json!({"optimisticMessageId":"o"})).unwrap();
        rpc!(
            SendAck,
            client.sessions().send_message(
                "s",
                "hello",
                SendParts {
                    options: Some(&options),
                    ..Default::default()
                }
            ),
            "sessions:sendMessage",
            json!(["s","hello",[],[],{"optimisticMessageId":"o"}]),
            json!({"accepted":true,"messageId":"m"})
        );
        rpc!(
            (),
            client.sessions().cancel("s", None),
            "sessions:cancel",
            json!(["s"]),
            Value::Null
        );
        rpc!(
            bool,
            client.sessions().respond_permission("s", "r", true, false),
            "sessions:respondToPermission",
            json!(["s", "r", true, false]),
            json!(true)
        );
        rpc!(
            CommandResult,
            client.sessions().command("s", &g::SessionCommand::Flag),
            "sessions:command",
            json!(["s",{"type":"flag"}]),
            Value::Null
        );
        rpc!(
            Option<g::PermissionModeState>,
            client.sessions().permission_mode("s"),
            "sessions:getPermissionModeState",
            json!(["s"]),
            Value::Null
        );
        rpc!(
            String,
            client.files().read_text("/plan.md"),
            "file:read",
            json!(["/plan.md"]),
            json!("# Plan")
        );
        rpc!(
            Option<g::PendingPlanExecution>,
            client.sessions().pending_plan("s"),
            "sessions:getPendingPlanExecution",
            json!(["s"]),
            Value::Null
        );
        rpc!(
            g::UnreadSummary,
            client.sessions().unread_summary(),
            "sessions:getUnreadSummary",
            json!([]),
            json!({"totalUnreadSessions":0,"byWorkspace":{},"hasUnreadByWorkspace":{}})
        );
        rpc!(
            (),
            client.sessions().mark_all_read("w"),
            "sessions:markAllRead",
            json!(["w"]),
            Value::Null
        );
        rpc!(
            Vec<g::SessionSearchResult>,
            client.sessions().search("w", "needle", None),
            "sessions:searchContent",
            json!(["w", "needle"]),
            json!([])
        );
        rpc!(
            Option<g::BackgroundTaskOutput>,
            client.sessions().background_output("s", "task", None),
            "sessions:getBackgroundTaskOutput",
            json!(["s", "task"]),
            Value::Null
        );
        rpc!(
            ActionResult,
            client.sessions().stop_background("s", "task"),
            "sessions:stopBackgroundTask",
            json!(["s", "task"]),
            json!({"success":true})
        );
        rpc!(
            ActionResult,
            client.sessions().kill_shell("s", "shell"),
            "sessions:killShell",
            json!(["s", "shell"]),
            json!({"success":true})
        );
        rpc!(
            Vec<g::SessionFile>,
            client.sessions().files("s"),
            "sessions:getFiles",
            json!(["s"]),
            json!([])
        );
        rpc!(
            (),
            client.sessions().watch_files("s"),
            "sessions:watchFiles",
            json!(["s"]),
            Value::Null
        );
        rpc!(
            (),
            client.sessions().unwatch_files(),
            "sessions:unwatchFiles",
            json!([]),
            Value::Null
        );
        let exported = rpc!(
            g::SessionBundle,
            client.sessions().export("s"),
            "sessions:export",
            json!(["s"]),
            bundle.clone()
        );
        rpc!(
            ImportResult,
            client.sessions().import("w", &exported, ImportMode::Fork),
            "sessions:import",
            json!(["w", bundle.clone(), "fork"]),
            json!({"sessionId":"s"})
        );
        rpc!(
            Option<String>,
            client.sessions().model("s", "w"),
            "session:getModel",
            json!(["s", "w"]),
            Value::Null
        );
        rpc!(
            (),
            client.sessions().set_model("s", "w", None, None),
            "session:setModel",
            json!(["s", "w", null]),
            Value::Null
        );
        rpc!(
            Option<g::SessionDraft>,
            client.drafts().get("s"),
            "drafts:get",
            json!(["s"]),
            Value::Null
        );
        let draft: g::SessionDraft = serde_json::from_value(json!({"text":"draft"})).unwrap();
        rpc!(
            (),
            client.drafts().set("s", &draft),
            "drafts:set",
            json!(["s",{"text":"draft"}]),
            Value::Null
        );
        rpc!(
            (),
            client.drafts().delete("s"),
            "drafts:delete",
            json!(["s"]),
            Value::Null
        );
        let file: g::FileAttachment = serde_json::from_value(
            json!({"type":"text","path":"/tmp/f","name":"f","mimeType":"text/plain","size":1}),
        )
        .unwrap();
        rpc!(
            g::StoredAttachment,
            client.files().store_attachment("s", &file),
            "file:storeAttachment",
            json!(["s",{"type":"text","path":"/tmp/f","name":"f","mimeType":"text/plain","size":1}]),
            json!({"id":"a","type":"text","name":"f","mimeType":"text/plain","size":1,"storedPath":"/tmp/f"})
        );
        rpc!(
            Option<g::FileAttachment>,
            client.files().read_attachment("/tmp/f"),
            "file:readAttachment",
            json!(["/tmp/f"]),
            Value::Null
        );
        rpc!(
            String,
            client.files().read_data_url("/tmp/f"),
            "file:readDataUrl",
            json!(["/tmp/f"]),
            json!("data:text/plain;base64,QQ==")
        );
        rpc!(
            String,
            client.files().read_preview_data_url("/tmp/f", None),
            "file:readPreviewDataUrl",
            json!(["/tmp/f"]),
            json!("data:image/png;base64,QQ==")
        );
        rpc!(
            Vec<g::FileSearchResult>,
            client.files().search("/tmp", "f"),
            "fs:search",
            json!(["/tmp", "f"]),
            json!([])
        );
        rpc!(
            g::DirectoryListingResult,
            client.files().list_directory("/tmp"),
            "fs:listDirectory",
            json!(["/tmp"]),
            json!({"currentPath":"/tmp","parentPath":null,"breadcrumbs":[],"platform":"linux","truncated":false,"totalEntries":0,"entries":[]})
        );
        rpc!(
            Vec<g::LoadedSkill>,
            client.skills().list("w", None),
            "skills:get",
            json!(["w"]),
            json!([])
        );
        rpc!(
            Vec<g::SkillFile>,
            client.skills().files("w", "skill"),
            "skills:getFiles",
            json!(["w", "skill"]),
            json!([])
        );
        rpc!(
            (),
            client.skills().delete("w", "skill"),
            "skills:delete",
            json!(["w", "skill"]),
            Value::Null
        );
        rpc!(
            Vec<g::LlmConnectionWithStatus>,
            client.connections().list_with_status(),
            "LLM_Connection:listWithStatus",
            json!([]),
            json!([])
        );
        let config: g::LlmConnection = serde_json::from_value(connection.clone()).unwrap();
        rpc!(
            ActionResult,
            client.connections().save(&config),
            "LLM_Connection:save",
            json!([connection.clone()]),
            json!({"success":true})
        );
        rpc!(
            ActionResult,
            client.connections().delete("test"),
            "LLM_Connection:delete",
            json!(["test"]),
            json!({"success":true})
        );
        rpc!(
            ActionResult,
            client.connections().test("test"),
            "LLM_Connection:test",
            json!(["test"]),
            json!({"success":true})
        );
        rpc!(
            ActionResult,
            client.connections().set_default("test"),
            "LLM_Connection:setDefault",
            json!(["test"]),
            json!({"success":true})
        );
        rpc!(
            ActionResult,
            client.connections().set_workspace_default("w", None),
            "LLM_Connection:setWorkspaceDefault",
            json!(["w", null]),
            json!({"success":true})
        );
        rpc!(
            ActionResult,
            client.connections().refresh_models("test"),
            "LLM_Connection:refreshModels",
            json!(["test"]),
            json!({"success":true})
        );
        let setup = ConnectionSetup {
            slug: "test".into(),
            credential: Some("fake-secret".into()),
            ..Default::default()
        };
        rpc!(
            ActionResult,
            client.connections().setup(&setup),
            "settings:setupLlmConnection",
            json!([{"slug":"test","credential":"fake-secret"}]),
            json!({"success":true})
        );
        let test = TestConnectionParams::new("fake-secret".into());
        rpc!(
            ActionResult,
            client.connections().test_setup(&test),
            "settings:testLlmConnectionSetup",
            json!([{"provider":"pi","apiKey":"fake-secret"}]),
            json!({"success":true})
        );
        rpc!(
            Vec<Provider>,
            client.connections().api_key_providers(),
            "pi:getApiKeyProviders",
            json!([]),
            json!([])
        );
        rpc!(
            Option<String>,
            client.connections().provider_base_url("pi"),
            "pi:getProviderBaseUrl",
            json!(["pi"]),
            Value::Null
        );
        rpc!(
            ProviderModels,
            client.connections().provider_models("pi"),
            "pi:getProviderModels",
            json!(["pi"]),
            json!({"models":[],"totalCount":0})
        );
        rpc!(
            ChatGptFlow,
            client.oauth().start_chatgpt("test"),
            "chatgpt:startOAuth",
            json!(["test"]),
            json!({"authUrl":"https://example.test","state":"state","flowId":"flow"})
        );
        let complete = ChatGptComplete {
            code: "code".into(),
            state: "state".into(),
            flow_id: "flow".into(),
        };
        rpc!(
            ActionResult,
            client.oauth().complete_chatgpt(&complete),
            "chatgpt:completeOAuth",
            json!([{"code":"code","state":"state","flowId":"flow"}]),
            json!({"success":true})
        );
        rpc!(
            ActionResult,
            client.oauth().cancel_chatgpt(None),
            "chatgpt:cancelOAuth",
            json!([]),
            json!({"success":true})
        );
        rpc!(
            AuthStatus,
            client.oauth().chatgpt_status("test"),
            "chatgpt:getAuthStatus",
            json!(["test"]),
            json!({"authenticated":false})
        );
        rpc!(
            ActionResult,
            client.oauth().logout_chatgpt("test"),
            "chatgpt:logout",
            json!(["test"]),
            json!({"success":true})
        );
        rpc!(
            ClaudeStart,
            client.oauth().start_claude(),
            "onboarding:startClaudeOAuth",
            json!([]),
            json!({"success":true,"authUrl":"https://example.test"})
        );
        let code: secrecy::SecretString = "code".into();
        rpc!(
            ClaudeExchange,
            client.oauth().exchange_claude(&code, "test"),
            "onboarding:exchangeClaudeCode",
            json!(["code", "test"]),
            json!({"success":true,"token":"token"})
        );
        rpc!(
            ActionResult,
            client.oauth().clear_claude_state(),
            "onboarding:clearClaudeOAuthState",
            json!([]),
            json!({"success":true})
        );
        rpc!(
            AuthState,
            client.oauth().auth_state(),
            "onboarding:getAuthState",
            json!([]),
            json!({"setupNeeds":{"isFullyConfigured":false,"needsBillingConfig":true}})
        );
        rpc!(
            ActionResult,
            client.oauth().defer_setup(),
            "onboarding:deferSetup",
            json!([]),
            json!({"success":true})
        );
        rpc!(
            g::ThinkingLevel,
            client.settings().default_thinking(),
            "settings:getDefaultThinkingLevel",
            json!([]),
            json!("medium")
        );
        rpc!(
            ActionResult,
            client
                .settings()
                .set_default_thinking(g::ThinkingLevel::High),
            "settings:setDefaultThinkingLevel",
            json!(["high"]),
            json!({"success":true})
        );
        rpc!(
            Option<u32>,
            client.settings().auto_compact_window(),
            "settings:getAutoCompactWindow",
            json!([]),
            Value::Null
        );
        rpc!(
            ActionResult,
            client.settings().set_auto_compact_window(None),
            "settings:setAutoCompactWindow",
            json!([null]),
            json!({"success":true})
        );
        rpc!(
            SendMessageKey,
            client.settings().send_message_key(),
            "input:getSendMessageKey",
            json!([]),
            json!("enter")
        );
        rpc!(
            (),
            client
                .settings()
                .set_send_message_key(SendMessageKey::CmdEnter),
            "input:setSendMessageKey",
            json!(["cmd-enter"]),
            Value::Null
        );
        rpc!(
            bool,
            client.settings().keep_awake(),
            "power:getKeepAwake",
            json!([]),
            json!(false)
        );
        rpc!(
            (),
            client.settings().set_keep_awake(true),
            "power:setKeepAwake",
            json!([true]),
            Value::Null
        );
        rpc!(
            NetworkProxy,
            client.settings().network_proxy(),
            "settings:getNetworkProxy",
            json!([]),
            json!({"enabled":false})
        );
        let proxy = NetworkProxy::default();
        let updated = rpc!(
            NetworkProxyUpdate,
            client.settings().set_network_proxy(&proxy),
            "settings:setNetworkProxy",
            json!([{"enabled":false}]),
            json!({"restartRequired":true})
        );
        assert!(updated.restart_required);
        rpc!(
            (),
            client.settings().disable_browser(),
            "tools:setBrowserToolEnabled",
            json!([false]),
            Value::Null
        );
        rpc!(
            DefaultPermissions,
            client.settings().default_permissions(),
            "permissions:getDefaults",
            json!([]),
            json!({"config":null,"path":"/tmp/p"})
        );
    });
    drop(expected);
    client.close();
    let visited = server.join().unwrap();
    assert_eq!(visited.len(), 80);
    assert_eq!(
        visited,
        agentkit_client::api::V1_OPERATIONS
            .iter()
            .map(|op| op.channel())
            .collect()
    );
}

#[test]
fn present_optional_suffixes_and_command_return_families_keep_their_shapes() {
    let (urls, recv) = std::sync::mpsc::channel();
    let (expected, waiting) = async_channel::unbounded::<Expected>();
    let server = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            urls.send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            send(&mut ws,json!({"type":"handshake_ack","clientId":"optional","registeredChannels":agentkit_protocol::constants::channels::ALL})).await;
            while let Ok(case) = waiting.recv().await {
                let req = frame(&mut ws).await;
                assert_eq!(req["channel"], case.channel);
                assert_eq!(req["args"], case.args, "{}", case.channel);
                send(
                    &mut ws,
                    json!({"type":"response","id":req["id"],"result":case.result}),
                )
                .await;
            }
        })
    });
    let client = RpcClient::connect(ClientOptions::local(
        recv.recv().unwrap(),
        "key".into(),
        "w".into(),
    ));
    macro_rules! rpc {
        ($call:expr,$channel:literal,$args:expr,$result:expr) => {{
            expected
                .send(Expected {
                    channel: $channel,
                    args: $args,
                    result: $result,
                })
                .await
                .unwrap();
            $call.await.unwrap()
        }};
    }
    smol::block_on(async {
        rpc!(
            client
                .sessions()
                .send_message("s", "bare", SendParts::default()),
            "sessions:sendMessage",
            json!(["s", "bare"]),
            json!({"accepted":true,"messageId":"m"})
        );
        let file =
            json!({"type":"text","path":"/tmp/f","name":"f","mimeType":"text/plain","size":1});
        let files: Vec<g::FileAttachment> = serde_json::from_value(json!([file.clone()])).unwrap();
        rpc!(
            client.sessions().send_message(
                "s",
                "file",
                SendParts {
                    attachments: Some(&files),
                    ..Default::default()
                }
            ),
            "sessions:sendMessage",
            json!(["s", "file", [file.clone()]]),
            json!({"accepted":true,"messageId":"m"})
        );
        let stored = json!({"id":"a","type":"text","name":"f","mimeType":"text/plain","size":1,"storedPath":"/tmp/f"});
        let stored_files: Vec<g::StoredAttachment> =
            serde_json::from_value(json!([stored.clone()])).unwrap();
        rpc!(
            client.sessions().send_message(
                "s",
                "stored",
                SendParts {
                    stored_attachments: Some(&stored_files),
                    ..Default::default()
                }
            ),
            "sessions:sendMessage",
            json!(["s", "stored", [], [stored]]),
            json!({"accepted":true,"messageId":"m"})
        );
        let options: g::CreateSessionOptions =
            serde_json::from_value(json!({"name":"named"})).unwrap();
        rpc!(
            client.sessions().create("w", Some(&options)),
            "sessions:create",
            json!(["w",{"name":"named"}]),
            json!({"id":"s","workspaceId":"w","workspaceName":"W","lastMessageAt":0,"messages":[],"isProcessing":false})
        );
        rpc!(
            client.sessions().cancel("s", Some(false)),
            "sessions:cancel",
            json!(["s", false]),
            Value::Null
        );
        rpc!(
            client.sessions().search("w", "x", Some("")),
            "sessions:searchContent",
            json!(["w", "x", ""]),
            json!([])
        );
        rpc!(
            client
                .sessions()
                .background_output("s", "t", Some(TaskKind::Shell)),
            "sessions:getBackgroundTaskOutput",
            json!(["s", "t", "shell"]),
            Value::Null
        );
        rpc!(
            client.files().read_preview_data_url("p", Some(128)),
            "file:readPreviewDataUrl",
            json!(["p", 128]),
            json!("data:image/png;base64,AA==")
        );
        rpc!(
            client.skills().list("w", Some("/tmp")),
            "skills:get",
            json!(["w", "/tmp"]),
            json!([])
        );
        rpc!(
            client.sessions().set_model("s", "w", None, Some("c")),
            "session:setModel",
            json!(["s", "w", null, "c"]),
            Value::Null
        );
        let cancel = ChatGptCancel {
            state: Some("state".into()),
        };
        rpc!(
            client.oauth().cancel_chatgpt(Some(&cancel)),
            "chatgpt:cancelOAuth",
            json!([{"state":"state"}]),
            json!({"success":true})
        );
        rpc!(
            client
                .workspaces()
                .update("w", &WorkspaceSetting::Model(None)),
            "workspaceSettings:update",
            json!(["w", "model", null]),
            Value::Null
        );
        assert!(matches!(
            rpc!(
                client
                    .sessions()
                    .command("s", &g::SessionCommand::RefreshTitle),
                "sessions:command",
                json!(["s",{"type":"refreshTitle"}]),
                json!({"success":true,"title":"title"})
            ),
            CommandResult::Title(_)
        ));
        assert!(matches!(
            rpc!(
                client.sessions().command("s", &g::SessionCommand::CopyPath),
                "sessions:command",
                json!(["s",{"type":"copyPath"}]),
                json!({"success":true,"path":"/tmp"})
            ),
            CommandResult::Path(_)
        ));
        assert!(matches!(
            rpc!(
                client.sessions().command(
                    "s",
                    &g::SessionCommand::ContinueInNewSession { coding_mode: true }
                ),
                "sessions:command",
                json!(["s",{"type":"continueInNewSession","codingMode":true}]),
                json!({"sessionId":"continued"})
            ),
            CommandResult::Continued(_)
        ));
    });
    drop(expected);
    client.close();
    server.join().unwrap();
}

#[test]
fn policy_retries_only_reads_and_rejects_business_failures_and_unavailable_patches() {
    let (urls, recv) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            urls.send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            send(&mut ws,json!({"type":"handshake_ack","clientId":"policy","registeredChannels":["sessions:get","settings:setupLlmConnection","sessions:watchFiles","onboarding:startClaudeOAuth","power:getKeepAwake"]})).await;
            let first = frame(&mut ws).await;
            assert_eq!(first["channel"], "sessions:get");
            send(&mut ws,json!({"type":"error","id":first["id"],"error":{"code":"REQUEST_TIMEOUT","message":"retry read"}})).await;
            let retry = frame(&mut ws).await;
            assert_eq!(retry["channel"], "sessions:get");
            assert_ne!(retry["id"], first["id"]);
            send(
                &mut ws,
                json!({"type":"response","id":retry["id"],"result":[]}),
            )
            .await;
            let setup = frame(&mut ws).await;
            assert_eq!(setup["channel"], "settings:setupLlmConnection");
            send(&mut ws,json!({"type":"response","id":setup["id"],"result":{"success":false,"error":"denied"}})).await;
            for channel in ["sessions:watchFiles", "onboarding:startClaudeOAuth"] {
                let request = frame(&mut ws).await;
                assert_eq!(request["channel"], channel);
                send(&mut ws,json!({"type":"error","id":request["id"],"error":{"code":"REQUEST_TIMEOUT","message":"do not retry mutation"}})).await;
                let next = frame(&mut ws).await;
                assert_eq!(next["channel"], "power:getKeepAwake");
                send(
                    &mut ws,
                    json!({"type":"response","id":next["id"],"result":false}),
                )
                .await;
            }
            let _ = bounded(ws.next()).await;
        })
    });
    let client = RpcClient::connect(ClientOptions::local(
        recv.recv().unwrap(),
        "key".into(),
        "w".into(),
    ));
    smol::block_on(async {
        assert!(client.sessions().list().await.unwrap().is_empty());
        let setup = ConnectionSetup {
            slug: "test".into(),
            ..Default::default()
        };
        assert_eq!(
            client
                .connections()
                .setup(&setup)
                .await
                .unwrap_err()
                .protocol_code(),
            "HANDLER_ERROR"
        );
        assert_eq!(
            client
                .sessions()
                .watch_files("s")
                .await
                .unwrap_err()
                .protocol_code(),
            "REQUEST_TIMEOUT"
        );
        assert!(!client.settings().keep_awake().await.unwrap());
        assert_eq!(
            client
                .oauth()
                .start_claude()
                .await
                .unwrap_err()
                .protocol_code(),
            "REQUEST_TIMEOUT"
        );
        assert!(!client.settings().keep_awake().await.unwrap());
        assert_eq!(
            client
                .settings()
                .set_keep_awake(true)
                .await
                .unwrap_err()
                .protocol_code(),
            "CHANNEL_NOT_FOUND"
        );
    });
    client.close();
    server.join().unwrap();
    for (channel, seconds, read) in [
        ("sessions:sendMessage", 300, false),
        ("file:storeAttachment", 120, false),
        ("sessions:searchContent", 15, true),
        ("sessions:export", 120, true),
        ("sessions:import", 120, false),
    ] {
        let op = agentkit_client::api::V1_OPERATIONS
            .iter()
            .find(|op| op.channel() == channel)
            .unwrap();
        assert_eq!(op.timeout().as_secs(), seconds);
        assert_eq!(op.is_read_only(), read);
    }
}

#[test]
fn grouped_facade_cannot_move_a_deferred_bound_write_to_a_new_socket() {
    let (urls, recv) = std::sync::mpsc::channel();
    let (drop_old, when) = async_channel::bounded::<()>(1);
    let server = std::thread::spawn(move || {
        smol::block_on(async move {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            urls.send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            send(&mut ws,json!({"type":"handshake_ack","clientId":"A","registeredChannels":["server:getHealth","sessions:sendMessage"]})).await;
            let req = frame(&mut ws).await;
            send(
                &mut ws,
                json!({"type":"response","id":req["id"],"result":{"status":"ok","checks":[]}}),
            )
            .await;
            when.recv().await.unwrap();
            ws.close(None).await.unwrap();
            let (tcp, _) = bounded(listener.accept()).await.unwrap().unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            send(&mut ws,json!({"type":"handshake_ack","clientId":"B","stale":true,"registeredChannels":["server:getHealth","sessions:sendMessage"]})).await;
            let req = frame(&mut ws).await;
            let moved = req["channel"] == "sessions:sendMessage";
            let result = if moved {
                json!({"accepted":true,"messageId":"bad"})
            } else {
                json!({"status":"ok","checks":[]})
            };
            send(
                &mut ws,
                json!({"type":"response","id":req["id"],"result":result}),
            )
            .await;
            moved
        })
    });
    let mut options = ClientOptions::local(recv.recv().unwrap(), "key".into(), "w".into());
    options.max_reconnect_delay = std::time::Duration::from_millis(20);
    let client = RpcClient::connect(options);
    let recovery = client.subscribe("__transport:reconnected");
    smol::block_on(async {
        client.service().health().await.unwrap();
        let (bound, _) = client.bind_connection().unwrap();
        let sessions = bound.sessions();
        let deferred = sessions.send_message("s", "old intent", SendParts::default());
        drop_old.send(()).await.unwrap();
        bounded(recovery.recv()).await.unwrap().unwrap();
        let result = deferred.await;
        if result.is_err() {
            client.service().health().await.unwrap();
        }
        assert!(matches!(
            result,
            Err(agentkit_client::RpcError::Disconnected)
        ));
    });
    client.close();
    assert!(!server.join().unwrap());
}

#[test]
fn plan_dispatch_command_preserves_both_claim_outcomes() {
    let (urls, recv) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        smol::block_on(async {
            let listener = async_net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            urls.send(format!("ws://{}", listener.local_addr().unwrap()))
                .unwrap();
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = async_tungstenite::accept_async(tcp).await.unwrap();
            let _ = frame(&mut ws).await;
            send(&mut ws, json!({"type":"handshake_ack","clientId":"plan","registeredChannels":["sessions:command"]})).await;
            // Source contract: the first claim succeeds, repeated/not-ready claims return false.
            for claimed in [true, false] {
                let request = frame(&mut ws).await;
                assert_eq!(request["channel"], "sessions:command");
                assert_eq!(
                    request["args"],
                    json!(["s", {"type":"markPendingPlanExecutionDispatched"}])
                );
                send(
                    &mut ws,
                    json!({"type":"response","id":request["id"],"result":claimed}),
                )
                .await;
            }
            while ws.next().await.is_some() {}
        });
    });
    let client = RpcClient::connect(ClientOptions::local(
        recv.recv().unwrap(),
        "key".into(),
        "w".into(),
    ));
    smol::block_on(async {
        for claimed in [true, false] {
            let result = client
                .sessions()
                .command("s", &g::SessionCommand::MarkPendingPlanExecutionDispatched)
                .await
                .unwrap();
            assert!(matches!(result, CommandResult::PlanDispatched(actual) if actual == claimed));
        }
    });
    client.close();
    server.join().unwrap();
}
