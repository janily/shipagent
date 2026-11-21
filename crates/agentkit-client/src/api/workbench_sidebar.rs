use super::*;
impl WorkbenchApi {
    /// Export persisted session history and its portable files.
    pub async fn export_session_bundle(&self, id: &str) -> Result<g::SessionBundle, RpcError> {
        self.client.sessions().export(id).await
    }

    /// Import a portable session using the selected backend transfer mode.
    pub async fn import_session_bundle(
        &self,
        workspace: &str,
        bundle: &g::SessionBundle,
        mode: super::super::types::ImportMode,
    ) -> Result<super::super::types::ImportResult, RpcError> {
        self.client.sessions().import(workspace, bundle, mode).await
    }

    /// Change the server workspace and future reconnect handshake together.
    pub async fn switch_workspace(&self, id: &str) -> Result<(), RpcError> {
        self.client.workspaces().switch(id).await
    }
    /// Validate the selected folder's slug before registering a workspace.
    pub async fn check_workspace_slug(
        &self,
        slug: &str,
    ) -> Result<super::super::SlugCheck, RpcError> {
        self.client.workspaces().check_slug(slug).await
    }
    /// Register a local directory as a workspace.
    pub async fn create_workspace(
        &self,
        folder: &str,
        name: &str,
    ) -> Result<g::Workspace, RpcError> {
        self.client.workspaces().create(folder, name).await
    }
    /// Unregister an inactive workspace; a false response remains a business failure.
    pub async fn remove_workspace(&self, id: &str) -> Result<(), RpcError> {
        if self.client.workspaces().remove(id).await? {
            Ok(())
        } else {
            Err(super::super::business_error(
                "Workspace is still in use".into(),
            ))
        }
    }
    /// Rename a persisted conversation, leaving messages unchanged.
    pub async fn rename_session(&self, id: &str, name: &str) -> Result<(), RpcError> {
        self.client
            .sessions()
            .command(id, &g::SessionCommand::Rename { name: name.into() })
            .await
            .map(|_| ())
    }
    /// Delete one explicitly confirmed conversation.
    pub async fn delete_session(&self, id: &str) -> Result<(), RpcError> {
        self.client.sessions().delete(id).await
    }
}
