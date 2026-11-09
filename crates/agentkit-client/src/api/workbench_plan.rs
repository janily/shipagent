use super::*;
impl WorkbenchApi {
    /// Load a canonical plan file through backend workspace path validation.
    pub async fn plan_text(&self, path: &str) -> Result<String, RpcError> {
        self.client.files().read_text(path).await
    }
    /// Read persisted readiness after opening, compaction completion or idle.
    pub async fn pending_plan(
        &self,
        id: &str,
    ) -> Result<Option<g::PendingPlanExecution>, RpcError> {
        self.client.sessions().pending_plan(id).await
    }
    /// Persist acceptance and the current draft before sending /compact.
    pub async fn prepare_plan(&self, id: &str, path: &str, draft: &str) -> Result<(), RpcError> {
        self.client
            .sessions()
            .command(
                id,
                &g::SessionCommand::SetPendingPlanExecution {
                    plan_path: path.into(),
                    draft_input_snapshot: Some(draft.into()),
                },
            )
            .await?;
        Ok(())
    }
    /// Only an explicit true from the atomic host operation grants dispatch ownership.
    pub async fn claim_plan(&self, id: &str) -> Result<bool, RpcError> {
        Ok(matches!(
            self.client
                .sessions()
                .command(id, &g::SessionCommand::MarkPendingPlanExecutionDispatched)
                .await?,
            crate::api::types::CommandResult::PlanDispatched(true)
        ))
    }
    /// Clear only after the claimed approval send has been acknowledged.
    pub async fn clear_plan(&self, id: &str) -> Result<(), RpcError> {
        self.client
            .sessions()
            .command(id, &g::SessionCommand::ClearPendingPlanExecution)
            .await?;
        Ok(())
    }
}
