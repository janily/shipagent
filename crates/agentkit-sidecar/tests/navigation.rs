#![allow(clippy::unwrap_used)]
//! Real typed sidebar operations and workspace binding isolation against the bundled backend.
use agentkit_client::api::WorkbenchApi;
use agentkit_sidecar::{SidecarConfig, SidecarHandle};
use std::time::Duration;
#[test]
#[ignore = "Requires AGENTKIT_BACKEND_DIR bundled backend"]
fn real_session_lifecycle_and_workspace_switch_remove_reopen_keep_data() {
    smol::block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("navigation-project");
        std::fs::create_dir(&folder).unwrap();
        let sidecar = SidecarHandle::start(SidecarConfig::new(
            std::env::var_os("AGENTKIT_BACKEND_DIR").unwrap().into(),
            temp.path().join("config"),
        ))
        .await
        .unwrap();
        let api = WorkbenchApi::connect(sidecar.endpoint(), String::new());
        api.health().await.unwrap();
        let original = api.workspaces_generated().await.unwrap()[0].clone();
        api.switch_workspace(&original.id).await.unwrap();
        let original_session = api
            .create_session_generated(&original.id, None)
            .await
            .unwrap();
        let check = api
            .check_workspace_slug("navigation-project")
            .await
            .unwrap();
        assert!(!check.exists);
        let workspace = api
            .create_workspace(folder.to_str().unwrap(), "Navigation project")
            .await
            .unwrap();
        assert!(
            api.workspaces_generated()
                .await
                .unwrap()
                .iter()
                .any(|w| w.id == workspace.id)
        );
        api.switch_workspace(&workspace.id).await.unwrap();
        let session = api
            .create_session_generated(&workspace.id, folder.to_str())
            .await
            .unwrap();
        assert_eq!(session.workspace_id, workspace.id);
        api.rename_session(&session.id, "Renamed 原生会话")
            .await
            .unwrap();
        assert_eq!(
            api.session_generated(&session.id)
                .await
                .unwrap()
                .name
                .as_deref(),
            Some("Renamed 原生会话")
        );
        let list = api.sessions_generated().await.unwrap();
        assert!(list.iter().all(|s| s.workspace_id == workspace.id));
        assert!(list.iter().all(|s| s.id != original_session.id));
        assert!(
            api.remove_workspace(&workspace.id).await.is_err(),
            "active workspace cannot be removed"
        );
        api.switch_workspace(&original.id).await.unwrap();
        assert!(
            api.sessions_generated()
                .await
                .unwrap()
                .iter()
                .all(|s| s.workspace_id == original.id)
        );
        api.remove_workspace(&workspace.id).await.unwrap();
        assert!(folder.exists());
        assert!(
            !api.workspaces_generated()
                .await
                .unwrap()
                .iter()
                .any(|w| w.id == workspace.id)
        );
        let reopened = api
            .create_workspace(folder.to_str().unwrap(), "Reopened")
            .await
            .unwrap();
        assert_eq!(workspace.id, reopened.id);
        api.switch_workspace(&reopened.id).await.unwrap();
        assert_eq!(
            api.session_generated(&session.id)
                .await
                .unwrap()
                .name
                .as_deref(),
            Some("Renamed 原生会话")
        );
        api.delete_session(&session.id).await.unwrap();
        assert!(
            api.sessions_generated()
                .await
                .unwrap()
                .iter()
                .all(|s| s.id != session.id)
        );
        api.switch_workspace(&original.id).await.unwrap();
        api.delete_session(&original_session.id).await.unwrap();
        api.remove_workspace(&reopened.id).await.unwrap();
        api.close();
        sidecar.shutdown(Duration::from_secs(5)).await.unwrap();
    });
}
