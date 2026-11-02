//! Typed files RPC operations.
use super::*;
pub struct Files<'a>(pub(super) &'a RpcClient);
impl Files<'_> {
    pub async fn store_attachment(
        &self,
        id: &str,
        file: &g::FileAttachment,
    ) -> Result<g::StoredAttachment, RpcError> {
        execute(
            self.0,
            p::FILES_STORE_ATTACHMENT,
            vec![json!(id), serde_json::to_value(file)?],
        )
        .await
    }
    pub async fn read_attachment(&self, path: &str) -> Result<Option<g::FileAttachment>, RpcError> {
        execute(self.0, p::FILES_READ_ATTACHMENT, vec![json!(path)]).await
    }
    pub async fn read_data_url(&self, path: &str) -> Result<String, RpcError> {
        execute(self.0, p::FILES_READ_DATA_URL, vec![json!(path)]).await
    }
    pub async fn read_preview_data_url(
        &self,
        path: &str,
        max_size: Option<u32>,
    ) -> Result<String, RpcError> {
        let mut args = vec![json!(path)];
        optional(&mut args, max_size)?;
        execute(self.0, p::FILES_READ_PREVIEW_DATA_URL, args).await
    }
    pub async fn search(
        &self,
        base: &str,
        query: &str,
    ) -> Result<Vec<g::FileSearchResult>, RpcError> {
        execute(self.0, p::FILES_SEARCH, vec![json!(base), json!(query)]).await
    }
    pub async fn list_directory(&self, path: &str) -> Result<g::DirectoryListingResult, RpcError> {
        execute(self.0, p::FILES_LIST_DIRECTORY, vec![json!(path)]).await
    }
}
