//! Typed turn/activity models from mkagent v0.2.6 turn-utils.ts and TurnCard.tsx.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Lifecycle of an activity row.
pub enum ActivityStatus {
    /// Pending state or payload.
    Pending,
    /// Running state or payload.
    Running,
    /// Completed state or payload.
    Completed,
    /// Error state or payload.
    Error,
    /// Backgrounded state or payload.
    Backgrounded,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Kind of activity rendered inside an assistant card.
pub enum ActivityType {
    /// Tool state or payload.
    Tool,
    /// Thinking state or payload.
    Thinking,
    /// Intermediate state or payload.
    Intermediate,
    /// Status state or payload.
    Status,
    /// Plan state or payload.
    Plan,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Derived assistant turn lifecycle phase, in display priority order.
pub enum TurnPhase {
    /// Pending state or payload.
    Pending,
    /// ToolActive state or payload.
    ToolActive,
    /// Awaiting state or payload.
    Awaiting,
    /// Streaming state or payload.
    Streaming,
    /// Complete state or payload.
    Complete,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// A projected tool, status, commentary or plan row.
pub struct Activity {
    /// Stable backing message or activity ID.
    pub id: String,
    #[serde(rename = "type")]
    /// Activity display kind.
    pub kind: ActivityType,
    /// Activity lifecycle or TodoWrite status.
    pub status: ActivityStatus,
    /// Ordering time in integral Unix milliseconds.
    pub timestamp: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Tool protocol name.
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Tool invocation ID used for hierarchy lookup.
    pub tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Tool-owned structured input.
    pub tool_input: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Display text or tool output.
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Human-readable intent provided by the model.
    pub intent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Authoritative backing message ID.
    pub message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Persisted annotations from the generated Message contract.
    pub annotations: Option<Vec<g::MessageAnnotationsItem>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Human-readable tool or plan name.
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Embedded generated tool metadata.
    pub tool_display_meta: Option<g::MessageToolDisplayMeta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Clean error text for display.
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Parent tool invocation ID.
    pub parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Nesting depth; group children use relative depths.
    pub depth: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Status protocol tag, such as compacting.
    pub status_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Background agent task ID.
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Background shell ID.
    pub shell_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Latest elapsed task time.
    pub elapsed_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Whether the activity runs in the background.
    pub is_background: Option<bool>,
}
impl Activity {
    pub(crate) fn new(
        id: String,
        kind: ActivityType,
        status: ActivityStatus,
        timestamp: i64,
    ) -> Self {
        Self {
            id,
            kind,
            status,
            timestamp,
            tool_name: None,
            tool_use_id: None,
            tool_input: None,
            content: None,
            intent: None,
            message_id: None,
            annotations: None,
            display_name: None,
            tool_display_meta: None,
            error: None,
            parent_id: None,
            depth: None,
            status_type: None,
            task_id: None,
            shell_id: None,
            elapsed_seconds: None,
            is_background: None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Final or currently streamed assistant response with backing message identity.
pub struct ResponseContent {
    /// Response text.
    pub text: String,
    /// Whether text or work is still streaming.
    pub is_streaming: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Start time of the currently streaming response.
    pub stream_start_time: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Whether the response represents a plan.
    pub is_plan: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Authoritative backing message ID.
    pub message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Persisted annotations from the generated Message contract.
    pub annotations: Option<Vec<g::MessageAnnotationsItem>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// TodoWrite input projection; status strings are owned by the tool protocol.
pub struct TodoItem {
    /// Display text or tool output.
    pub content: String,
    /// Activity lifecycle or TodoWrite status.
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Present continuous form of an active todo.
    pub active_form: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// One assistant card bounded by users, final answers or plans.
pub struct AssistantTurn {
    /// Fixed serialized discriminator.
    #[serde(rename = "type")]
    pub kind: AssistantTurnKind,
    /// SDK turn ID or initial backing message ID; not the UI card key.
    pub turn_id: String,
    /// Chronologically ordered activity rows.
    pub activities: Vec<Activity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Final or streamed response, if present.
    pub response: Option<ResponseContent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Human-readable intent provided by the model.
    pub intent: Option<String>,
    /// Whether text or work is still streaming.
    pub is_streaming: bool,
    /// Whether the assistant card is complete.
    pub is_complete: bool,
    /// Ordering time in integral Unix milliseconds.
    pub timestamp: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Latest valid TodoWrite state.
    pub todos: Option<Vec<TodoItem>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Visible user message and its ordering timestamp.
pub struct UserTurn {
    /// Fixed serialized discriminator.
    #[serde(rename = "type")]
    pub kind: UserTurnKind,
    /// Unmodified generated backing message.
    pub message: g::Message,
    /// Ordering time in integral Unix milliseconds.
    pub timestamp: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Standalone error, warning or informational message.
pub struct SystemTurn {
    /// Fixed serialized discriminator.
    #[serde(rename = "type")]
    pub kind: SystemTurnKind,
    /// Unmodified generated backing message.
    pub message: g::Message,
    /// Ordering time in integral Unix milliseconds.
    pub timestamp: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
/// Chronological chat card variants.
pub enum Turn {
    /// Assistant state or payload.
    Assistant(AssistantTurn),
    /// User state or payload.
    User(UserTurn),
    /// System state or payload.
    System(SystemTurn),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Numeric background TaskOutput usage and duration.
pub struct TaskOutputData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Task duration in milliseconds.
    pub duration_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Task input token usage.
    pub input_tokens: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Task output token usage.
    pub output_tokens: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Subagent parent, sorted descendants and optional background usage.
pub struct ActivityGroup {
    /// Fixed serialized discriminator.
    #[serde(rename = "type")]
    pub kind: ActivityGroupKind,
    /// Parent subagent row or continued-group stand-in.
    pub parent: Activity,
    /// All descendants sorted by time with relative depth.
    pub children: Vec<Activity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Usage merged from completed TaskOutput.
    pub task_output_data: Option<TaskOutputData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// True for a subagent started in an earlier turn.
    pub continued: Option<bool>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
/// Standalone row or a subagent activity group.
pub enum ActivityOrGroup {
    /// Group state or payload.
    Group(ActivityGroup),
    /// Activity state or payload.
    Activity(Activity),
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Caller-provided session lifecycle used to finalize an open turn.
pub struct GroupTurnsOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Explicit false finalizes an open activity turn after session completion.
    pub is_session_processing: Option<bool>,
}

/// Fixed discriminator for AssistantTurn.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AssistantTurnKind {
    /// Assistant payload.
    #[serde(rename = "assistant")]
    Assistant,
}

/// Fixed discriminator for UserTurn.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum UserTurnKind {
    /// User payload.
    #[serde(rename = "user")]
    User,
}

/// Fixed discriminator for SystemTurn.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SystemTurnKind {
    /// System payload.
    #[serde(rename = "system")]
    System,
}

/// Fixed discriminator for ActivityGroup.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ActivityGroupKind {
    /// Group payload.
    #[serde(rename = "group")]
    Group,
}
