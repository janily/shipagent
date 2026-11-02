//! Typed sessions RPC operations.
use super::*;
pub struct Sessions<'a>(pub(super) &'a RpcClient);
impl Sessions<'_> {
    pub async fn list(&self) -> Result<Vec<g::Session>, RpcError> {
        execute(self.0, p::SESSIONS_LIST, vec![]).await
    }
    pub async fn get_messages(&self, id: &str) -> Result<Option<g::Session>, RpcError> {
        execute(self.0, p::SESSIONS_GET_MESSAGES, vec![json!(id)]).await
    }
    pub async fn create(
        &self,
        workspace: &str,
        options: Option<&g::CreateSessionOptions>,
    ) -> Result<g::Session, RpcError> {
        let mut args = vec![json!(workspace)];
        optional(&mut args, options)?;
        execute(self.0, p::SESSIONS_CREATE, args).await
    }
    pub async fn delete(&self, id: &str) -> Result<(), RpcError> {
        execute(self.0, p::SESSIONS_DELETE, vec![json!(id)]).await
    }
    pub async fn send_message(
        &self,
        id: &str,
        text: &str,
        parts: SendParts<'_>,
    ) -> Result<SendAck, RpcError> {
        let mut args = vec![json!(id), json!(text)];
        if parts.attachments.is_some()
            || parts.stored_attachments.is_some()
            || parts.options.is_some()
        {
            args.push(serde_json::to_value(parts.attachments.unwrap_or(&[]))?);
        }
        if parts.stored_attachments.is_some() || parts.options.is_some() {
            args.push(serde_json::to_value(
                parts.stored_attachments.unwrap_or(&[]),
            )?);
        }
        optional(&mut args, parts.options)?;
        execute(self.0, p::SESSIONS_SEND_MESSAGE, args).await
    }
    pub async fn cancel(&self, id: &str, silent: Option<bool>) -> Result<(), RpcError> {
        let mut args = vec![json!(id)];
        optional(&mut args, silent)?;
        execute(self.0, p::SESSIONS_CANCEL, args).await
    }
    pub async fn respond_permission(
        &self,
        id: &str,
        request: &str,
        allowed: bool,
        always: bool,
    ) -> Result<bool, RpcError> {
        execute(
            self.0,
            p::SESSIONS_RESPOND_PERMISSION,
            vec![json!(id), json!(request), json!(allowed), json!(always)],
        )
        .await
    }
    pub async fn command(
        &self,
        id: &str,
        command: &g::SessionCommand,
    ) -> Result<CommandResult, RpcError> {
        let command = serde_json::to_value(command)?;
        let kind = command["type"].as_str().unwrap_or("").to_owned();
        let value: Value = execute(self.0, p::SESSIONS_COMMAND, vec![json!(id), command]).await?;
        Ok(match kind.as_str() {
            "refreshTitle" => {
                let result: g::RefreshTitleResult = serde_json::from_value(value)?;
                if !result.success {
                    return Err(business_error(
                        result
                            .error
                            .unwrap_or_else(|| "Title refresh failed".into()),
                    ));
                }
                CommandResult::Title(result)
            }
            "copyPath" => {
                let result: CopyPathResult = serde_json::from_value(value)?;
                if !result.success {
                    return Err(business_error("Session path unavailable".into()));
                }
                CommandResult::Path(result)
            }
            "continueInNewSession" => CommandResult::Continued(serde_json::from_value(value)?),
            _ => {
                let _: () = serde_json::from_value(value)?;
                CommandResult::Empty
            }
        })
    }
    pub async fn permission_mode(
        &self,
        id: &str,
    ) -> Result<Option<g::PermissionModeState>, RpcError> {
        execute(self.0, p::SESSIONS_PERMISSION_MODE, vec![json!(id)]).await
    }
    pub async fn pending_plan(
        &self,
        id: &str,
    ) -> Result<Option<g::PendingPlanExecution>, RpcError> {
        execute(self.0, p::SESSIONS_PENDING_PLAN, vec![json!(id)]).await
    }
    pub async fn unread_summary(&self) -> Result<g::UnreadSummary, RpcError> {
        execute(self.0, p::SESSIONS_UNREAD_SUMMARY, vec![]).await
    }
    pub async fn mark_all_read(&self, workspace: &str) -> Result<(), RpcError> {
        execute(self.0, p::SESSIONS_MARK_ALL_READ, vec![json!(workspace)]).await
    }
    pub async fn search(
        &self,
        workspace: &str,
        query: &str,
        search_id: Option<&str>,
    ) -> Result<Vec<g::SessionSearchResult>, RpcError> {
        let mut args = vec![json!(workspace), json!(query)];
        optional(&mut args, search_id)?;
        execute(self.0, p::SESSIONS_SEARCH, args).await
    }
    pub async fn background_output(
        &self,
        id: &str,
        task: &str,
        kind: Option<TaskKind>,
    ) -> Result<Option<g::BackgroundTaskOutput>, RpcError> {
        let mut args = vec![json!(id), json!(task)];
        optional(&mut args, kind)?;
        execute(self.0, p::SESSIONS_BACKGROUND_OUTPUT, args).await
    }
    pub async fn stop_background(&self, id: &str, task: &str) -> Result<ActionResult, RpcError> {
        action(
            self.0,
            p::SESSIONS_STOP_BACKGROUND,
            vec![json!(id), json!(task)],
        )
        .await
    }
    pub async fn kill_shell(&self, id: &str, shell: &str) -> Result<ActionResult, RpcError> {
        action(
            self.0,
            p::SESSIONS_KILL_SHELL,
            vec![json!(id), json!(shell)],
        )
        .await
    }
    pub async fn files(&self, id: &str) -> Result<Vec<g::SessionFile>, RpcError> {
        execute(self.0, p::SESSIONS_FILES, vec![json!(id)]).await
    }
    pub async fn watch_files(&self, id: &str) -> Result<(), RpcError> {
        execute(self.0, p::SESSIONS_WATCH_FILES, vec![json!(id)]).await
    }
    pub async fn unwatch_files(&self) -> Result<(), RpcError> {
        execute(self.0, p::SESSIONS_UNWATCH_FILES, vec![]).await
    }
    pub async fn export(&self, id: &str) -> Result<g::SessionBundle, RpcError> {
        execute(self.0, p::SESSIONS_EXPORT, vec![json!(id)]).await
    }
    pub async fn import(
        &self,
        workspace: &str,
        bundle: &g::SessionBundle,
        mode: ImportMode,
    ) -> Result<ImportResult, RpcError> {
        execute(
            self.0,
            p::SESSIONS_IMPORT,
            vec![json!(workspace), serde_json::to_value(bundle)?, json!(mode)],
        )
        .await
    }
    pub async fn model(&self, id: &str, workspace: &str) -> Result<Option<String>, RpcError> {
        execute(self.0, p::SESSIONS_MODEL, vec![json!(id), json!(workspace)]).await
    }
    pub async fn set_model(
        &self,
        id: &str,
        workspace: &str,
        model: Option<&str>,
        connection: Option<&str>,
    ) -> Result<(), RpcError> {
        let mut args = vec![json!(id), json!(workspace), json!(model)];
        optional(&mut args, connection)?;
        execute(self.0, p::SESSIONS_SET_MODEL, args).await
    }
}
