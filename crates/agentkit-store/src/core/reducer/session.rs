//! Dispatch and session handlers ported from mkagent v0.2.6 event-processor.
use super::*;
/// Ported from mkagent v0.2.6 apps/electron/src/renderer/event-processor/processor.ts and handlers/session.ts.
pub(super) fn apply(
    out: &mut ProcessResult,
    event: &g::SessionEvent,
    ctx: &Context,
) -> Result<(), ReducerError> {
    use g::SessionEvent as E;
    let state = &mut out.state;
    match event {
        E::RetryBackoffEvent(e) => {
            clear_retry(&mut state.session);
            let mut m = message(ctx, g::MessageRole::Status, e.message.clone());
            m.status_type = Some(g::MessageStatusType::Retrying);
            append(&mut state.session, m, false, ctx);
            state.session.current_status = Some(g::SessionCurrentStatus {
                message: e.message.clone(),
                status_type: Some("retrying".into()),
            });
        }
        E::RetryStateEvent(_) => clear_retry(&mut state.session),
        E::CompleteEvent(e) => {
            clear_retry(&mut state.session);
            for m in &mut state.session.messages {
                if m.role == g::MessageRole::Tool
                    && !matches!(
                        m.tool_status,
                        Some(g::ToolStatus::Completed | g::ToolStatus::Error)
                    )
                    && !(m.is_background == Some(true)
                        && m.task_id.as_ref().is_some_and(|s| !s.is_empty()))
                {
                    m.tool_status = Some(g::ToolStatus::Completed);
                    if m.tool_result.is_none() {
                        m.tool_result = Some(String::new());
                    }
                }
                if m.role == g::MessageRole::User && m.is_queued == Some(true) {
                    m.is_queued = Some(false);
                }
            }
            state.session.is_processing = false;
            state.session.current_status = None;
            state.streaming = None;
            if let Some(usage) = &e.token_usage {
                let mut updated: g::SessionTokenUsage = convert(usage)?;
                if updated.context_usage.is_none() {
                    updated.context_usage = state
                        .session
                        .token_usage
                        .as_ref()
                        .and_then(|u| u.context_usage.clone());
                }
                state.session.token_usage = Some(updated);
            }
            if let Some(unread) = e.has_unread {
                state.session.has_unread = Some(unread);
            }
        }
        E::ErrorEvent(e) => {
            fail_tools(&mut state.session);
            let mut m = message(ctx, g::MessageRole::Error, e.error.clone());
            m.timestamp = time(e.timestamp, ctx.now_ms)?;
            state.session.messages.push(m);
            state.session.is_processing = false;
            state.session.current_status = None;
            state.streaming = None;
        }
        E::TypedErrorEvent(e) => {
            fail_tools(&mut state.session);
            let err = &e.error;
            let content = if err.title.is_empty() {
                err.message.clone()
            } else {
                format!("{}: {}", err.title, err.message)
            };
            let mut m = message(ctx, g::MessageRole::Error, content);
            m.timestamp = time(e.timestamp, ctx.now_ms)?;
            m.error_code = Some(err.code.to_string());
            m.error_title = Some(err.title.clone());
            m.error_details = err.details.clone();
            m.error_original = err.original_error.clone();
            m.error_can_retry = Some(err.can_retry);
            m.error_actions = Some(convert(&err.actions)?);
            state.session.messages.push(m);
            state.session.is_processing = false;
            state.session.current_status = None;
            state.streaming = None;
        }
        E::StatusEvent(e) => {
            let mut m = message(ctx, g::MessageRole::Status, e.message.clone());
            m.status_type = convert(&e.status_type)?;
            append(&mut state.session, m, false, ctx);
            state.session.current_status = Some(g::SessionCurrentStatus {
                message: e.message.clone(),
                status_type: e.status_type.as_ref().map(ToString::to_string),
            });
        }
        E::InfoEvent(e) => {
            if e.status_type
                .as_ref()
                .is_some_and(|s| s.to_string() == "compaction_complete")
            {
                for m in &mut state.session.messages {
                    if m.role == g::MessageRole::Status
                        && m.status_type == Some(g::MessageStatusType::Compacting)
                    {
                        m.role = g::MessageRole::Info;
                        m.content = e.message.clone();
                        m.status_type = Some(g::MessageStatusType::CompactionComplete);
                        m.info_level = convert(&e.level)?;
                    }
                }
                state.session.current_status = None;
            } else {
                let mut m = message(ctx, g::MessageRole::Info, e.message.clone());
                m.timestamp = time(e.timestamp, ctx.now_ms)?;
                m.info_level = convert(&e.level)?;
                append(&mut state.session, m, false, ctx);
            }
        }
        E::InterruptedEvent(e) => {
            let user = e.message.is_some();
            state.session.messages.retain(|m| {
                m.role != g::MessageRole::Status && !(user && m.is_queued == Some(true))
            });
            for m in &mut state.session.messages {
                if m.role == g::MessageRole::Tool
                    && m.tool_result.is_none()
                    && !matches!(
                        m.tool_status,
                        Some(g::ToolStatus::Completed | g::ToolStatus::Error)
                    )
                {
                    m.tool_status = Some(g::ToolStatus::Error);
                    m.tool_result = Some("Interrupted".into());
                    m.is_error = Some(true);
                } else if m.role == g::MessageRole::Assistant && m.is_pending == Some(true) {
                    m.is_pending = Some(false);
                    m.is_streaming = Some(false);
                }
            }
            if let Some(m) = &e.message {
                state.session.messages.push(m.clone());
            }
            if user && let Some(queued) = e.queued_messages.as_ref().filter(|q| !q.is_empty()) {
                out.effects.push(Effect::RestoreInput {
                    text: queued.join("\n\n"),
                });
            }
            state.session.is_processing = false;
            state.session.current_status = None;
            state.streaming = None;
        }
        E::TitleGeneratedEvent(e) => {
            state.session.name = Some(e.title.clone());
            state.session.is_regenerating_title = Some(false);
        }
        E::TitleRegeneratingEvent(e) => {
            state.session.is_regenerating_title = Some(e.is_regenerating)
        }
        E::AsyncOperationEvent(e) => state.session.is_async_operation_ongoing = Some(e.is_ongoing),
        E::WorkingDirectoryChangedEvent(e) => {
            state.session.working_directory = Some(e.working_directory.clone())
        }
        E::WorkingDirectoryErrorEvent(e) => out.effects.push(Effect::ToastError {
            message: e.error.clone(),
        }),
        E::CodingModeChangedEvent(e) => {
            state.session.coding_mode = if e.coding_mode { Some(true) } else { None };
            out.effects.push(Effect::CodingModeChanged {
                session_id: e.session_id.clone(),
                coding_mode: e.coding_mode,
            });
        }
        E::PermissionModeChangedEvent(e) => out.effects.push(Effect::PermissionModeChanged {
            session_id: e.session_id.clone(),
            permission_mode: e.permission_mode,
            previous_permission_mode: e.previous_permission_mode,
            transition_display: e.transition_display.clone(),
            mode_version: e.mode_version,
            changed_at: e.changed_at.clone(),
            changed_by: e.changed_by,
        }),
        E::SessionModelChangedEvent(e) => state.session.model = e.model.clone(),
        E::ConnectionChangedEvent(e) => {
            state.session.llm_connection = Some(e.connection_slug.clone());
            if let Some(branching) = e.supports_branching {
                state.session.supports_branching = Some(branching);
            }
        }
        E::UserMessageEvent(e) => user_message(state, e, ctx),
        E::MessageAnnotationsUpdatedEvent(e) => {
            if let Some(m) = state
                .session
                .messages
                .iter_mut()
                .find(|m| m.id == e.message_id)
            {
                m.annotations = Some(convert(&e.annotations)?);
            }
        }
        E::UsageUpdateEvent(e) => {
            let old = state.session.token_usage.as_ref();
            let u = &e.token_usage;
            state.session.token_usage = Some(g::SessionTokenUsage {
                input_tokens: u.input_tokens,
                output_tokens: old.map_or(0.0, |u| u.output_tokens),
                total_tokens: old.map_or(0.0, |u| u.total_tokens),
                context_tokens: old.map_or(0.0, |u| u.context_tokens),
                cost_usd: old.map_or(0.0, |u| u.cost_usd),
                cache_read_tokens: old.and_then(|u| u.cache_read_tokens),
                cache_creation_tokens: old.and_then(|u| u.cache_creation_tokens),
                context_window: u
                    .context_window
                    .or_else(|| old.and_then(|u| u.context_window)),
                context_usage: match &u.context_usage {
                    Some(context) => Some(convert(context)?),
                    None => old.and_then(|u| u.context_usage.clone()),
                },
            });
        }
        E::SessionFlaggedEvent(_) => state.session.is_flagged = Some(true),
        E::SessionUnflaggedEvent(_) => state.session.is_flagged = Some(false),
        E::SessionArchivedEvent(_) => {
            state.session.is_archived = Some(true);
            state.session.archived_at = Some(ctx.now_ms);
        }
        E::SessionUnarchivedEvent(_) => {
            state.session.is_archived = Some(false);
            state.session.archived_at = None;
        }
        E::NameChangedEvent(e) => state.session.name = e.name.clone(),
        E::PermissionRequestEvent(e) => out.effects.push(Effect::PermissionRequest {
            request: e.request.clone(),
        }),
        E::PlanSubmittedEvent(e) => append(&mut state.session, e.message.clone(), false, ctx),
        // Routing-only events have no upstream message/session mutation.
        E::SessionCreatedEvent(_)
        | E::SessionDeletedEvent(_)
        | E::ShellKilledEvent(_)
        | E::WorkflowAgentCompletedEvent(_)
        | E::TaskActivityEvent(_) => {}
        E::TextDeltaEvent(e) => text::delta(state, e, ctx),
        E::TextCompleteEvent(e) => text::complete(state, e, ctx)?,
        E::TextDiscardEvent(e) => text::discard(state, e),
        E::ToolStartEvent(e) => tool::start(state, e, ctx)?,
        E::ToolResultEvent(e) => tool::result(state, e, ctx)?,
        E::TaskBackgroundedEvent(e) => tool::task_backgrounded(state, e),
        E::ShellBackgroundedEvent(e) => tool::shell_backgrounded(state, e),
        E::TaskProgressEvent(e) => tool::progress(state, e),
        E::TaskCompletedEvent(e) => tool::completed(state, e),
    }
    Ok(())
}
/// Ported from mkagent v0.2.6 apps/electron/src/renderer/event-processor/handlers/session.ts.
fn fail_tools(session: &mut g::Session) {
    clear_retry(session);
    for m in &mut session.messages {
        if m.role == g::MessageRole::Tool
            && m.tool_result.is_none()
            && !matches!(
                m.tool_status,
                Some(g::ToolStatus::Completed | g::ToolStatus::Error)
            )
        {
            m.tool_status = Some(g::ToolStatus::Error);
            m.tool_result = Some("Error occurred".into());
            m.is_error = Some(true);
        }
    }
}
/// Ported from mkagent v0.2.6 apps/electron/src/renderer/event-processor/handlers/session.ts.
fn user_message(state: &mut SessionState, e: &g::SessionUserMessageEvent, ctx: &Context) {
    let status = e.status.to_string();
    let queued = status == "queued";
    let index = state.session.messages.iter().position(|m| {
        m.role == g::MessageRole::User
            && (m.id == e.message.id
                || e.optimistic_message_id
                    .as_ref()
                    .is_some_and(|id| id == &m.id)
                || (m.content == e.message.content
                    && m.timestamp.abs_diff(e.message.timestamp) < 5000))
    });
    if let Some(i) = index {
        let m = &mut state.session.messages[i];
        if queued && m.is_queued == Some(false) {
            return;
        }
        if status == "processing" {
            m.timestamp = e.message.timestamp;
        }
        m.is_pending = Some(false);
        m.is_queued = Some(queued);
    } else {
        let mut m = e.message.clone();
        m.is_pending = Some(false);
        m.is_queued = Some(queued);
        state.session.messages.push(m);
    }
    state.session.last_message_at = ctx.now_ms;
    state.session.last_message_role = Some(g::SessionLastMessageRole::User);
    state.session.is_processing = status == "accepted" || status == "processing";
}
