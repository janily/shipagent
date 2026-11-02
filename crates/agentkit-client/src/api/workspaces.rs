//! Typed workspaces RPC operations.
use super::*;
pub struct Workspaces<'a>(pub(super) &'a RpcClient);
impl Workspaces<'_> {
    pub async fn list(&self) -> Result<Vec<g::Workspace>, RpcError> {
        execute(self.0, p::WORKSPACES_LIST, vec![]).await
    }
    pub async fn create(&self, folder: &str, name: &str) -> Result<g::Workspace, RpcError> {
        execute(
            self.0,
            p::WORKSPACES_CREATE,
            vec![json!(folder), json!(name)],
        )
        .await
    }
    pub async fn check_slug(&self, slug: &str) -> Result<SlugCheck, RpcError> {
        execute(self.0, p::WORKSPACES_CHECK_SLUG, vec![json!(slug)]).await
    }
    pub async fn remove(&self, id: &str) -> Result<bool, RpcError> {
        execute(self.0, p::WORKSPACES_REMOVE, vec![json!(id)]).await
    }
    pub async fn switch(&self, id: &str) -> Result<(), RpcError> {
        self.0
            .switch_workspace_timeout(id, p::WORKSPACES_SWITCH.timeout())
            .await
    }
    pub async fn settings(&self, id: &str) -> Result<Option<g::WorkspaceSettings>, RpcError> {
        execute(self.0, p::WORKSPACES_SETTINGS, vec![json!(id)]).await
    }
    pub async fn update(&self, id: &str, setting: &WorkspaceSetting) -> Result<(), RpcError> {
        let (key, value) = setting.wire();
        execute(
            self.0,
            p::WORKSPACES_UPDATE,
            vec![json!(id), json!(key), value],
        )
        .await
    }
    pub async fn permissions(&self, id: &str) -> Result<Option<PermissionsConfig>, RpcError> {
        execute(self.0, p::WORKSPACES_PERMISSIONS, vec![json!(id)]).await
    }
}
