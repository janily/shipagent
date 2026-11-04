//! Explicit local fixture preparation through the real typed backend; never opens a user profile.
use agentkit_client::api::WorkbenchApi;
use agentkit_sidecar::{SidecarConfig, SidecarHandle};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    smol::block_on(async {
        let args: Vec<_> = std::env::args().collect();
        if args.len() != 4 || !args[3].starts_with("http://127.0.0.1:") {
            return Err("Usage: navigation-fixture NEW_ROOT BACKEND LOOPBACK_MODEL_URL".into());
        }
        let root = std::path::PathBuf::from(&args[1]);
        if root.join("data").exists() {
            return Err("Fixture profile already exists".into());
        }
        std::fs::create_dir_all(&root)?;
        let sidecar = SidecarHandle::start(SidecarConfig::new(
            args[2].clone().into(),
            root.join("data"),
        ))
        .await?;
        let api = WorkbenchApi::connect(sidecar.endpoint(), String::new());
        api.health().await?;
        api.setup_endpoint(&args[3], "fixture-model", None).await?;
        let first = api
            .workspaces_generated()
            .await?
            .into_iter()
            .next()
            .ok_or("No workspace")?;
        api.switch_workspace(&first.id).await?;
        let folder = root.join("fixture");
        std::fs::create_dir_all(&folder)?;
        std::fs::write(
            folder.join("proof.txt"),
            "SHIPAGENT_TOOL_FIXTURE: native sidebar\n",
        )?;
        for index in 0..1000 {
            let s = api
                .create_session_generated(&first.id, folder.to_str())
                .await?;
            api.rename_session(&s.id, &format!("Sidebar task {index:04}"))
                .await?;
        }
        let mut summaries = api.sessions_generated().await?;
        summaries.sort_by(|a, b| {
            b.last_message_at
                .cmp(&a.last_message_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        let selected = summaries.first().map(|s| s.id.clone());
        let second_folder = root.join("second");
        std::fs::create_dir_all(&second_folder)?;
        let second = api
            .create_workspace(
                second_folder.to_str().ok_or("Invalid path")?,
                "Workspace Two",
            )
            .await?;
        // Keep the second workspace empty: native switching must create its first session and release refresh ownership.
        api.switch_workspace(&first.id).await?;
        let result = serde_json::json!({"workspace":first.id,"session":selected,"other":second.id,"sessions":1000});
        std::fs::write(
            root.join("sidebar-seed.json"),
            serde_json::to_vec_pretty(&result)?,
        )?;
        api.close();
        sidecar.shutdown(std::time::Duration::from_secs(5)).await?;
        Ok(())
    })
}
