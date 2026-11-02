//! Typed skills RPC operations.
use super::*;
pub struct Skills<'a>(pub(super) &'a RpcClient);
impl Skills<'_> {
    pub async fn list(
        &self,
        workspace: &str,
        working_directory: Option<&str>,
    ) -> Result<Vec<g::LoadedSkill>, RpcError> {
        let mut args = vec![json!(workspace)];
        optional(&mut args, working_directory)?;
        execute(self.0, p::SKILLS_LIST, args).await
    }
    pub async fn files(&self, workspace: &str, slug: &str) -> Result<Vec<g::SkillFile>, RpcError> {
        execute(self.0, p::SKILLS_FILES, vec![json!(workspace), json!(slug)]).await
    }
    pub async fn delete(&self, workspace: &str, slug: &str) -> Result<(), RpcError> {
        execute(
            self.0,
            p::SKILLS_DELETE,
            vec![json!(workspace), json!(slug)],
        )
        .await
    }
}
