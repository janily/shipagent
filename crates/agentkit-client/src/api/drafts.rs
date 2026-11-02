//! Typed drafts RPC operations.
use super::*;
pub struct Drafts<'a>(pub(super) &'a RpcClient);
impl Drafts<'_> {
    pub async fn get(&self, id: &str) -> Result<Option<g::SessionDraft>, RpcError> {
        execute(self.0, p::DRAFTS_GET, vec![json!(id)]).await
    }
    pub async fn set(&self, id: &str, draft: &g::SessionDraft) -> Result<(), RpcError> {
        execute(
            self.0,
            p::DRAFTS_SET,
            vec![json!(id), serde_json::to_value(draft)?],
        )
        .await
    }
    pub async fn delete(&self, id: &str) -> Result<(), RpcError> {
        execute(self.0, p::DRAFTS_DELETE, vec![json!(id)]).await
    }
}
