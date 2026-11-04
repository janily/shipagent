//! Prepare a new isolated native-settings fixture through the ordinary backend API.
use agentkit_client::api::WorkbenchApi;
use agentkit_sidecar::{SidecarConfig, SidecarHandle};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    smol::block_on(async {
        let args: Vec<_> = std::env::args().collect();
        if args.len() != 4 || !args[3].starts_with("http://127.0.0.1:") {
            return Err("Usage: settings-fixture NEW_ROOT BACKEND LOOPBACK_MODEL".into());
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
        let workspace = api
            .workspaces_generated()
            .await?
            .into_iter()
            .next()
            .ok_or("No workspace")?;
        api.switch_workspace(&workspace.id).await?;
        let session = api.create_session_generated(&workspace.id, None).await?;
        std::fs::write(
            root.join("settings-seed.json"),
            serde_json::to_vec(
                &serde_json::json!({"workspace":workspace.id,"session":session.id}),
            )?,
        )?;
        api.close();
        sidecar.shutdown(std::time::Duration::from_secs(5)).await?;
        Ok(())
    })
}
