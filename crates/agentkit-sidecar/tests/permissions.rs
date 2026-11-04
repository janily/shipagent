#![allow(clippy::unwrap_used)]
//! Real Agent approval decisions and remembered grants, driven by an isolated local model.
use agentkit_client::api::WorkbenchApi;
use agentkit_protocol::generated as g;
use agentkit_sidecar::{SidecarConfig, SidecarHandle};
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    time::Duration,
};
struct Model(Child);
impl Drop for Model {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[test]
#[ignore = "Requires bundled backend and Node; AGENTKIT_BACKEND_DIR"]
fn real_approvals_allow_deny_and_remember_without_repeating_the_prompt() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let temp = tempfile::tempdir().unwrap();
    let mut model = Model(
        Command::new("node")
            .arg(root.join("tools/permissions/model.mjs"))
            .env("AGENTKIT_FIXTURE_ROOT", temp.path())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut line = String::new();
    BufReader::new(model.0.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let url = line
        .trim()
        .strip_prefix("FAKE_LLM_URL=")
        .unwrap()
        .to_owned();
    smol::block_on(async {
        let backend = std::env::var_os("AGENTKIT_BACKEND_DIR").unwrap();
        let sidecar = SidecarHandle::start(SidecarConfig::new(
            backend.into(),
            temp.path().join("config"),
        ))
        .await
        .unwrap();
        let initial = WorkbenchApi::connect(sidecar.endpoint().clone(), String::new());
        initial.health().await.unwrap();
        initial
            .setup_endpoint(&url, "fixture-model", None)
            .await
            .unwrap();
        let workspaces = initial.workspaces_generated().await.unwrap();
        initial.close();
        let api = WorkbenchApi::connect(sidecar.endpoint().clone(), workspaces[0].id.clone());
        api.health().await.unwrap();
        let events = api.session_events();
        for (allowed, always) in [(true, false), (false, false), (true, true)] {
            let session = api
                .create_session_generated(&workspaces[0].id, temp.path().to_str())
                .await
                .unwrap();
            api.set_session_mode(&session.id, g::PermissionMode::Ask)
                .await
                .unwrap();
            for round in 0..if always { 2 } else { 1 } {
                let path = temp.path().join("decision.txt");
                let _ = std::fs::remove_file(&path);
                let ack = api
                    .send(
                        &session.id,
                        "Write decision.txt",
                        &format!("user-{allowed}-{always}-{round}"),
                    )
                    .await
                    .unwrap();
                assert!(ack.accepted);
                let counts = smol::future::race(
                    async {
                        let mut prompts = 0;
                        let mut results = 0;
                        loop {
                            let args = events.recv().await.unwrap();
                            let e: agentkit_protocol::DecodedSessionEvent =
                                serde_json::from_value(args[0].clone()).unwrap();
                            if args[0]["sessionId"].as_str() != Some(session.id.as_str()) {
                                continue;
                            }
                            match e.known() {
                                Some(g::SessionEvent::PermissionRequestEvent(e)) => {
                                    prompts += 1;
                                    if always {
                                        assert_eq!(e.request.can_always_allow, Some(true));
                                    }
                                    api.respond_permission(
                                        &session.id,
                                        &e.request.request_id,
                                        allowed,
                                        always,
                                    )
                                    .await
                                    .unwrap();
                                }
                                Some(g::SessionEvent::ToolResultEvent(_)) => results += 1,
                                Some(g::SessionEvent::CompleteEvent(_)) => {
                                    break (prompts, results);
                                }
                                Some(
                                    g::SessionEvent::ErrorEvent(_)
                                    | g::SessionEvent::TypedErrorEvent(_),
                                ) => panic!("approval round failed"),
                                _ => {}
                            }
                        }
                    },
                    async {
                        smol::Timer::after(Duration::from_secs(20)).await;
                        panic!("approval round timed out")
                    },
                )
                .await;
                assert_eq!(counts.0, if always && round == 1 { 0 } else { 1 });
                assert_eq!(counts.1, 1);
                assert_eq!(path.exists(), allowed);
                assert!(
                    !api.session_generated(&session.id)
                        .await
                        .unwrap()
                        .is_processing
                );
            }
        }
        api.close();
        sidecar.shutdown(Duration::from_secs(5)).await.unwrap();
    });
}
