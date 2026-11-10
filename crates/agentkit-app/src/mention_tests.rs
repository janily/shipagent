#![allow(clippy::unwrap_used)]
use super::*;
use agentkit_store::core::mentions::Skill;
use std::prelude::v1::test;
struct Model(std::process::Child);
impl Drop for Model {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[test]
#[ignore = "Requires bundled backend and Node; isolated local model"]
fn real_mentions_search_send_and_restart() {
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
                .arg(repo.join("tools/fixtures/mentions-model.mjs"))
                .arg(repo.join("tools/fake-llm/scenarios/text.yaml"))
                .env("AGENTKIT_FIXTURE_ROOT", temp.path())
                .env(
                    "AGENTKIT_MENTION_EVIDENCE",
                    temp.path().join("requests.jsonl"),
                )
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
        let dir = temp.path().join("project");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/你好.rs"), "MENTION_FILE_MARKER").unwrap();
        let skill_dir = std::path::Path::new(&workspace.root_path).join("skills/review-code");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: Review\ndescription: Review code changes\n---\nMENTION_SKILL_MARKER",
        )
        .unwrap();
        let project_skill = dir.join(".agents/skills/project-review");
        std::fs::create_dir_all(&project_skill).unwrap();
        std::fs::write(
            project_skill.join("SKILL.md"),
            "---\nname: Project review\ndescription: Project skill\n---\nProject instructions",
        )
        .unwrap();
        let loaded = api
            .session_skills(&workspace.id, dir.to_str())
            .await
            .unwrap();
        assert!(
            loaded
                .iter()
                .any(|s| s.slug == "project-review" && s.source == g::LoadedSkillSource::Project)
        );
        assert!(
            !api.workspace_skills(&workspace.id)
                .await
                .unwrap()
                .iter()
                .any(|s| s.slug == "project-review")
        );
        let skills = loaded
            .iter()
            .map(|s| Skill::from_loaded(s, &workspace.slug))
            .collect::<Vec<_>>();
        assert!(skills.iter().any(|s| s.slug == "review-code"));
        let results = api
            .search_mentions(dir.to_str().unwrap(), "你好")
            .await
            .unwrap();
        assert!(
            results
                .iter()
                .any(|f| f.relative_path == "src/你好.rs"
                    && f.type_ == g::FileSearchResultType::File)
        );
        assert!(
            api.search_mentions(dir.to_str().unwrap(), "src")
                .await
                .unwrap()
                .iter()
                .any(|f| f.type_ == g::FileSearchResultType::Directory)
        );
        let s = api
            .create_session(&workspace.id, Some(dir.to_str().unwrap()))
            .await
            .unwrap();
        let text = format!(
            "😀 [file:src/你好.rs] [folder:src] [skill:{}:review-code]",
            workspace.slug
        );
        let options = mentions::metadata(&text, &skills);
        assert_eq!(options.badges.as_ref().unwrap()[0].start, 3.);
        for (id, prepared) in [("mentions-normal", false), ("mentions-prepared", true)] {
            let ack = if prepared {
                api.send_prepared_with_options(&s.id, &text, id, &[], &[], options.clone())
                    .await
            } else {
                api.send_with_attachments_with_options(&s.id, &text, id, &[], &[], options.clone())
                    .await
            }
            .unwrap();
            assert!(ack.accepted);
            let deadline = Instant::now() + Duration::from_secs(40);
            loop {
                let snapshot = api.session_generated(&s.id).await.unwrap();
                if !snapshot.is_processing
                    && snapshot
                        .messages
                        .iter()
                        .filter(|m| m.role == g::MessageRole::Assistant)
                        .count()
                        > if prepared { 1 } else { 0 }
                {
                    let message = snapshot
                        .messages
                        .iter()
                        .find(|m| m.id == ack.message_id)
                        .unwrap();
                    assert_eq!(message.content, text);
                    assert_eq!(
                        serde_json::to_value(&message.badges).unwrap(),
                        serde_json::to_value(&options.badges).unwrap()
                    );
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "mention turn did not finish: {:?}",
                    snapshot.messages
                );
                smol::Timer::after(Duration::from_millis(30)).await;
            }
        }
        let requests = std::fs::read_to_string(temp.path().join("requests.jsonl")).unwrap();
        assert!(
            requests.contains("Read these Skill files before acting:")
                && requests.contains("review-code"),
            "skill directive missing from actual provider input: {requests}"
        );
        assert!(
            requests.contains("你好.rs")
                && requests.contains("Mentioned file:")
                && requests.contains("Mentioned folder:"),
            "file path missing from actual provider input"
        );
        api.close();
        sidecar.shutdown(Duration::from_secs(5)).await.unwrap();
        let restarted = SidecarHandle::start(SidecarConfig::new(backend, config))
            .await
            .unwrap();
        let api = WorkbenchApi::connect(restarted.endpoint(), workspace.id);
        let snapshot = api.session_generated(&s.id).await.unwrap();
        assert_eq!(
            snapshot
                .messages
                .iter()
                .filter(|m| m.role == g::MessageRole::User
                    && m.badges.as_ref().is_some_and(|b| b.len() == 3))
                .count(),
            2
        );
        api.close();
        restarted.shutdown(Duration::from_secs(5)).await.unwrap();
    });
}
