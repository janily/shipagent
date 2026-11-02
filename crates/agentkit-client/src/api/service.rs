//! Typed service RPC operations.
use super::*;
pub struct Service<'a>(pub(super) &'a RpcClient);
impl Service<'_> {
    pub async fn health(&self) -> Result<g::ServerHealth, RpcError> {
        execute(self.0, p::SERVICE_HEALTH, vec![]).await
    }
    pub async fn versions(&self) -> Result<Versions, RpcError> {
        execute(self.0, p::SERVICE_VERSIONS, vec![]).await
    }
}
