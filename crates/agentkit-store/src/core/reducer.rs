//! Deterministic port of mkagent v0.2.6 renderer/event-processor/{processor,handlers,helpers}.
mod helpers;
mod session;
mod text;
pub(crate) use helpers::message;
pub(crate) use text::delta_at;
mod tool;
use agentkit_protocol::{DecodedSessionEvent, generated as g};
use helpers::*;
use serde::{Deserialize, Serialize};
/// Transient streaming text, separate from persisted message content.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Streaming {
    /// Accumulated unfinished text.
    pub content: String,
    /// Backend turn identity when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    /// Parent activity identity, retained across deltas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_tool_use_id: Option<String>,
}
/// Wire-complete session plus transient stream; no GPUI or transport ownership.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionState {
    /// Generated, validated backend snapshot.
    pub session: g::Session,
    /// Current unfinished text, explicitly null when idle.
    pub streaming: Option<Streaming>,
}
/// Explicit replacements for upstream Date.now() and global message ID allocation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    /// Caller-provided wall-clock milliseconds, fixed for this reduction.
    pub now_ms: i64,
    /// Fresh nonempty ID reserved for this event's single possible local message.
    pub message_id: String,
}
/// Effects are descriptions only; executing them belongs to application controllers.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Effect {
    /// Show a permission request without mutating the transcript.
    PermissionRequest {
        /// Validated request.
        request: g::PermissionRequest,
    },
    /// Request a generated session title (retained upstream effect contract).
    GenerateTitle {
        /// Session whose title changes.
        session_id: String,
        /// User text used for generation.
        user_message: String,
    },
    /// Update session options outside the reducer.
    CodingModeChanged {
        /// Session whose setting changes.
        session_id: String,
        /// New coding mode.
        coding_mode: bool,
    },
    /// Versioned permission-mode change, handled outside message state.
    PermissionModeChanged {
        /// Session identity.
        session_id: String,
        /// New mode.
        permission_mode: g::PermissionMode,
        /// Previous mode when provided.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        previous_permission_mode: Option<g::PermissionMode>,
        /// Backend display explanation.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transition_display: Option<String>,
        /// Monotonic settings version.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode_version: Option<f64>,
        /// Backend change timestamp.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        changed_at: Option<String>,
        /// Change origin.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        changed_by: Option<g::SessionPermissionModeChangedEventChangedBy>,
    },
    /// Restore queued input after an explicit user stop.
    RestoreInput {
        /// Text joined in queue order.
        text: String,
    },
    /// Present a backend error without altering history.
    ToastError {
        /// Backend explanation.
        message: String,
    },
}
/// Successful immutable reduction.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessResult {
    /// Next session/stream state.
    pub state: SessionState,
    /// Deferred application work, in upstream order.
    pub effects: Vec<Effect>,
}
/// A typed generated-DTO adapter could not preserve the wire contract.
#[derive(Debug, thiserror::Error)]
pub enum ReducerError {
    /// Generated nested types with equal wire schemas no longer agree.
    #[error("Reducer DTO projection failed")]
    Projection(#[from] serde_json::Error),
    /// Wire timestamps must fit signed integral milliseconds.
    #[error("Reducer event timestamp is not an integral i64")]
    InvalidTimestamp,
    /// Caller failed to reserve a usable message ID.
    #[error("Reducer context message ID is empty")]
    EmptyMessageId,
}
/// Reduce one validated event without IO, clocks, randomness or input mutation.
pub fn process(
    state: &SessionState,
    event: &g::SessionEvent,
    context: &Context,
) -> Result<ProcessResult, ReducerError> {
    if event_session(event) != state.session.id {
        return Ok(unchanged(state));
    }
    if context.message_id.is_empty() {
        return Err(ReducerError::EmptyMessageId);
    }
    let mut next = unchanged(state);
    session::apply(&mut next, event, context)?;
    Ok(next)
}
/// Future event tags remain available at ingress and leave the known state unchanged.
pub fn process_decoded(
    state: &SessionState,
    event: &DecodedSessionEvent,
    context: &Context,
) -> Result<ProcessResult, ReducerError> {
    match event.known() {
        Some(event) => process(state, event, context),
        None => Ok(unchanged(state)),
    }
}
fn unchanged(state: &SessionState) -> ProcessResult {
    ProcessResult {
        state: state.clone(),
        effects: vec![],
    }
}

pub(crate) fn event_session(event: &g::SessionEvent) -> &str {
    match event {
        g::SessionEvent::AsyncOperationEvent(e) => &e.session_id,
        g::SessionEvent::CodingModeChangedEvent(e) => &e.session_id,
        g::SessionEvent::CompleteEvent(e) => &e.session_id,
        g::SessionEvent::ConnectionChangedEvent(e) => &e.session_id,
        g::SessionEvent::ErrorEvent(e) => &e.session_id,
        g::SessionEvent::InfoEvent(e) => &e.session_id,
        g::SessionEvent::InterruptedEvent(e) => &e.session_id,
        g::SessionEvent::MessageAnnotationsUpdatedEvent(e) => &e.session_id,
        g::SessionEvent::NameChangedEvent(e) => &e.session_id,
        g::SessionEvent::PermissionModeChangedEvent(e) => &e.session_id,
        g::SessionEvent::PermissionRequestEvent(e) => &e.session_id,
        g::SessionEvent::PlanSubmittedEvent(e) => &e.session_id,
        g::SessionEvent::RetryBackoffEvent(e) => &e.session_id,
        g::SessionEvent::RetryStateEvent(e) => &e.session_id,
        g::SessionEvent::SessionArchivedEvent(e) => &e.session_id,
        g::SessionEvent::SessionCreatedEvent(e) => &e.session_id,
        g::SessionEvent::SessionDeletedEvent(e) => &e.session_id,
        g::SessionEvent::SessionFlaggedEvent(e) => &e.session_id,
        g::SessionEvent::SessionModelChangedEvent(e) => &e.session_id,
        g::SessionEvent::SessionUnarchivedEvent(e) => &e.session_id,
        g::SessionEvent::SessionUnflaggedEvent(e) => &e.session_id,
        g::SessionEvent::ShellBackgroundedEvent(e) => &e.session_id,
        g::SessionEvent::ShellKilledEvent(e) => &e.session_id,
        g::SessionEvent::StatusEvent(e) => &e.session_id,
        g::SessionEvent::TaskActivityEvent(e) => &e.session_id,
        g::SessionEvent::TaskBackgroundedEvent(e) => &e.session_id,
        g::SessionEvent::TaskCompletedEvent(e) => &e.session_id,
        g::SessionEvent::TaskProgressEvent(e) => &e.session_id,
        g::SessionEvent::TextCompleteEvent(e) => &e.session_id,
        g::SessionEvent::TextDeltaEvent(e) => &e.session_id,
        g::SessionEvent::TextDiscardEvent(e) => &e.session_id,
        g::SessionEvent::TitleGeneratedEvent(e) => &e.session_id,
        g::SessionEvent::TitleRegeneratingEvent(e) => &e.session_id,
        g::SessionEvent::ToolResultEvent(e) => &e.session_id,
        g::SessionEvent::ToolStartEvent(e) => &e.session_id,
        g::SessionEvent::TypedErrorEvent(e) => &e.session_id,
        g::SessionEvent::UsageUpdateEvent(e) => &e.session_id,
        g::SessionEvent::UserMessageEvent(e) => &e.session_id,
        g::SessionEvent::WorkflowAgentCompletedEvent(e) => &e.session_id,
        g::SessionEvent::WorkingDirectoryChangedEvent(e) => &e.session_id,
        g::SessionEvent::WorkingDirectoryErrorEvent(e) => &e.session_id,
    }
}
