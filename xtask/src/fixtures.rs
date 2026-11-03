use agentkit_client::{
    ClientOptions, RpcClient,
    api::{WorkbenchApi, types::SendParts},
};
use agentkit_protocol::generated as g;
use agentkit_sidecar::{SidecarConfig, SidecarHandle};
use agentkit_store::fixtures::{SCENARIOS, Snapshots};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
struct Model(Child);
impl Drop for Model {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
pub(crate) fn record(name: &str) -> Result<()> {
    ensure!(
        name == "all" || SCENARIOS.contains(&name),
        "Unknown fixture scenario"
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("workspace")?;
    let out = std::env::var_os("AGENTKIT_FIXTURE_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("crates/agentkit-store/tests/fixtures/recorded"));
    let sources: Value =
        serde_json::from_slice(&fs::read(root.join("tools/codegen/sources.json"))?)?;
    let revision = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root.join("backend/mkagent"))
        .output()?;
    ensure!(
        revision.status.success()
            && String::from_utf8(revision.stdout)?.trim()
                == sources["backendRevision"]
                    .as_str()
                    .context("pinned revision")?,
        "Backend checkout differs from reviewed revision"
    );
    fs::create_dir_all(&out)?;
    for scenario in SCENARIOS
        .iter()
        .copied()
        .filter(|s| name == "all" || *s == name)
    {
        capture(root, &out, scenario).with_context(|| format!("Fixture {scenario}"))?;
        println!("Recorded and replayed {scenario}");
    }
    Ok(())
}
fn capture(root: &Path, out: &Path, name: &str) -> Result<()> {
    let temp = tempfile::tempdir()?;
    fs::write(temp.path().join("proof.txt"), "FIXTURE_READ\n")?;
    fs::write(temp.path().join("second.txt"), "FIXTURE_SECOND\n")?;
    fs::write(
        temp.path().join("plan.md"),
        "# Fixture plan\n\n1. Inspect.\n2. Verify.\n",
    )?;
    let script_path = root.join(format!("tools/fake-llm/scenarios/{name}.yaml"));
    let script_bytes = fs::read(&script_path)?;
    let script: Value = serde_json::from_slice(&script_bytes)?;
    let prompt = script["prompt"]
        .as_str()
        .context("scenario prompt")?
        .to_owned();
    let model_bytes = fs::read(root.join("tools/fixtures/model.mjs"))?;
    let mut model = Model(
        Command::new(std::env::var_os("AGENTKIT_NODE").unwrap_or_else(|| "node".into()))
            .arg(root.join("tools/fixtures/model.mjs"))
            .arg(root.join(format!("tools/fake-llm/scenarios/{name}.yaml")))
            .env("AGENTKIT_FIXTURE_ROOT", temp.path())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?,
    );
    let stdout = model.0.stdout.take().context("model stdout")?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(stdout).read_line(&mut line);
        let _ = tx.send(result.map(|_| line));
    });
    let line = rx.recv_timeout(Duration::from_secs(10))??;
    let url = line
        .trim()
        .strip_prefix("FAKE_LLM_URL=")
        .context("model readiness")?
        .to_owned();
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        x => x,
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    };
    let backend = std::env::var_os("AGENTKIT_BACKEND_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(format!("target/backend/{platform}-{arch}")));
    smol::block_on(async {
        let sidecar = SidecarHandle::start(SidecarConfig::new(
            backend.clone(),
            temp.path().join("config"),
        ))
        .await?;
        let result = run(&sidecar, &url, temp.path(), name, &prompt).await;
        let cleanup = sidecar.shutdown(Duration::from_secs(5)).await;
        let (initial, events, final_state) = result?;
        cleanup?;
        ensure!(
            script_bytes == fs::read(&script_path)?
                && model_bytes == fs::read(root.join("tools/fixtures/model.mjs"))?,
            "Scenario source changed during capture"
        );
        if name.starts_with("permission-") {
            ensure!(
                temp.path().join("decision.txt").exists() == (name == "permission-allow"),
                "Permission decision did not match file side effect"
            );
        }
        let replace = |v: &Value| serde_json::to_string_pretty(v).map(|s| scrub(&s, temp.path()));
        let initial = replace(&serde_json::to_value(initial)?)?;
        let final_state = replace(&serde_json::to_value(final_state)?)?;
        let events = scrub(
            &(events
                .into_iter()
                .map(|v| serde_json::to_string(&v))
                .collect::<std::result::Result<Vec<_>, _>>()?
                .join("\n")
                + "\n"),
            temp.path(),
        );
        fs::create_dir_all(out.join(".pending"))?;
        fs::write(out.join(format!(".pending/{name}.initial.json")), &initial)?;
        fs::write(
            out.join(format!(".pending/{name}.final.json")),
            &final_state,
        )?;
        fs::write(out.join(format!(".pending/{name}.jsonl")), &events)?;
        let evidence =
            agentkit_store::fixtures::verify_scenario(name, &initial, &events, &final_state)
                .map_err(anyhow::Error::msg)?;
        agentkit_store::fixtures::replay(&initial, &events, &final_state)
            .map_err(anyhow::Error::msg)?;
        let digest = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
        let sources: Value =
            serde_json::from_slice(&fs::read(root.join("tools/codegen/sources.json"))?)?;
        let metadata = json!({"schemaVersion":1,"projectionVersion":1,"scenario":name,"upstreamTag":"v0.2.6","upstreamCommit":sources["backendRevision"],"platform":std::env::consts::OS,"modelSha256":digest(&fs::read(root.join("tools/fixtures/model.mjs"))?),"backendSha256":{"server":digest(&fs::read(backend.join("server.js"))?),"pi":digest(&fs::read(backend.join("resources/pi-agent-server/index.js"))?)},"scenarioSha256":digest(&fs::read(root.join(format!("tools/fake-llm/scenarios/{name}.yaml")))?),"files":{"initial":digest(initial.as_bytes()),"events":digest(events.as_bytes()),"final":digest(final_state.as_bytes())},"evidence":evidence});
        fs::write(
            out.join(format!("{name}.meta.json")),
            serde_json::to_string_pretty(&metadata)?,
        )?;
        fs::write(out.join(format!("{name}.initial.json")), initial)?;
        fs::write(out.join(format!("{name}.final.json")), final_state)?;
        fs::write(out.join(format!("{name}.jsonl")), events)?;
        for suffix in ["initial.json", "final.json", "jsonl"] {
            fs::remove_file(out.join(format!(".pending/{name}.{suffix}")))?;
        }
        Ok(())
    })
}
async fn send(client: &RpcClient, id: &str, text: &str) -> Result<()> {
    client
        .sessions()
        .send_message(
            id,
            text,
            SendParts {
                attachments: None,
                stored_attachments: None,
                options: None,
            },
        )
        .await?;
    Ok(())
}
async fn run(
    sidecar: &SidecarHandle,
    url: &str,
    path: &Path,
    name: &str,
    prompt: &str,
) -> Result<(Snapshots, Vec<Value>, Snapshots)> {
    let endpoint = sidecar.endpoint();
    let api = WorkbenchApi::connect(endpoint.clone(), String::new());
    api.setup_endpoint(url, "fixture-model", None).await?;
    api.disable_browser().await?;
    let workspace = api
        .workspaces_generated()
        .await?
        .into_iter()
        .next()
        .context("workspace")?;
    api.close();
    let client = RpcClient::connect(ClientOptions::local(
        endpoint.url,
        endpoint.token,
        workspace.id.clone(),
    ));
    let events = client.events().session();
    let options: g::CreateSessionOptions = serde_json::from_value(
        json!({"name":format!("Fixture {name}"),"workingDirectory":path,"permissionMode":if name.starts_with("permission-"){"ask"}else{"allow-all"}}),
    )?;
    let session = client
        .sessions()
        .create(&workspace.id, Some(&options))
        .await?;
    let id = session.id.clone();
    let mut initial = Snapshots::from([(id.clone(), session)]);
    send(&client, &id, prompt).await?;
    let mut captured = Vec::new();
    let mut complete = BTreeSet::new();
    let mut appended = false;
    let mut cancelled = false;
    let mut compacted = 0;
    let mut background = false;
    let deadline = Instant::now() + Duration::from_secs(70);
    loop {
        ensure!(Instant::now() < deadline, "Timed out recording {name}");
        let event = smol::future::race(async { events.recv().await.map(Some) }, async {
            smol::Timer::after(Duration::from_millis(30)).await;
            Ok(None)
        })
        .await?;
        if let Some(event) = event {
            let event = event?;
            let raw = serde_json::to_value(event)?;
            let sid = raw["sessionId"].as_str().context("sessionId")?.to_owned();
            if !initial.contains_key(&sid) {
                let mut s = client
                    .sessions()
                    .get_messages(&sid)
                    .await?
                    .context("created snapshot")?;
                s.messages.clear();
                s.is_processing = false;
                initial.insert(sid.clone(), s);
            }
            let tag = raw["type"].as_str().unwrap_or_default();
            if tag == "permission_request" {
                let request = raw["request"]["requestId"]
                    .as_str()
                    .context("permission id")?;
                ensure!(
                    client
                        .sessions()
                        .respond_permission(&sid, request, name == "permission-allow", false)
                        .await?,
                    "Permission response rejected"
                );
            }
            if tag == "text_delta" && name == "cancel" && !cancelled {
                cancelled = true;
                client.sessions().cancel(&id, None).await?;
            }
            if tag == "text_delta" && name == "append-message" && !appended {
                appended = true;
                send(&client, &id, "Appended fixture input.").await?;
            }
            if tag == "task_completed" {
                background = true;
            }
            if tag == "complete" || (tag == "interrupted" && name == "cancel") {
                complete.insert(sid.clone());
            }
            captured.push(raw);
        }
        if complete.contains(&id) {
            if name == "compaction" && compacted < 2 {
                compacted += 1;
                complete.remove(&id);
                let text = if compacted == 1 {
                    "Historical fixture context. ".repeat(6000)
                } else {
                    "/compact Preserve fixture evidence.".into()
                };
                send(&client, &id, &text).await?;
                continue;
            }
            if name == "nested-agent" && (initial.len() < 3 || complete.len() < 3) {
                continue;
            }
            if name == "background-shell" && !background {
                continue;
            }
            if name == "append-message"
                && captured
                    .iter()
                    .filter(|e| e["type"] == "user_message")
                    .count()
                    < 2
            {
                continue;
            }
            let mut finals = Snapshots::new();
            let mut busy = false;
            for sid in initial.keys() {
                let s = client
                    .sessions()
                    .get_messages(sid)
                    .await?
                    .context("final snapshot")?;
                busy |= s.is_processing;
                finals.insert(sid.clone(), s);
            }
            if busy || !events.is_empty() {
                continue;
            }
            client.close();
            return Ok((initial, captured, finals));
        }
    }
}

fn scrub(text: &str, path: &Path) -> String {
    let path = path.to_string_lossy();
    text.replace(&path.replace('\\', "\\\\"), "<FIXTURE_ROOT>")
        .replace(&path.replace('\\', "/"), "<FIXTURE_ROOT>")
}
pub(crate) fn replay_dir(dir: &Path) -> Result<()> {
    for name in SCENARIOS {
        let initial = fs::read_to_string(dir.join(format!("{name}.initial.json")))?;
        let events = fs::read_to_string(dir.join(format!("{name}.jsonl")))?;
        let final_state = fs::read_to_string(dir.join(format!("{name}.final.json")))?;
        let meta: Value =
            serde_json::from_slice(&fs::read(dir.join(format!("{name}.meta.json")))?)?;
        for (key, text) in [
            ("initial", &initial),
            ("events", &events),
            ("final", &final_state),
        ] {
            ensure!(
                meta["files"][key] == format!("{:x}", Sha256::digest(text.as_bytes())),
                "Fixture {name}: {key} checksum mismatch"
            );
        }
        let evidence =
            agentkit_store::fixtures::verify_scenario(name, &initial, &events, &final_state)
                .map_err(anyhow::Error::msg)?;
        ensure!(
            meta["schemaVersion"] == 1
                && meta["projectionVersion"] == 1
                && meta["scenario"] == *name
                && meta["evidence"] == evidence,
            "Fixture {name}: manifest mismatch"
        );
        agentkit_store::fixtures::replay(&initial, &events, &final_state)
            .map_err(anyhow::Error::msg)?;
    }
    println!("Replayed all14 recorded scenarios");
    Ok(())
}
