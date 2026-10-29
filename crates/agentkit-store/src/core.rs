//! Phase-zero subset ported from mkagent v0.2.6 renderer/event-processor/handlers.
use agentkit_protocol::prototype::{Message, PermissionRequest, Session, SessionEvent};
use serde_json::Value;
/// Conversation lifecycle drives button availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    /// Ready for another message.
    Idle,
    /// Backend is processing the current round.
    Processing,
    /// An explicit tool approval is required.
    WaitingForPermission,
    /// Round was cancelled or interrupted.
    Interrupted,
    /// Backend reported a failure.
    Failed,
}
/// Replayable phase-zero conversation state. No GPUI or network dependency.
#[derive(Debug, Clone)]
pub struct ChatState {
    /// Current backend session and message cache.
    pub session: Session,
    /// Current round state.
    pub status: RunState,
    /// Current permission request, when one exists.
    pub permission: Option<PermissionRequest>,
    /// Last operation failure, shown without discarding the conversation.
    pub last_error: Option<String>,
}
impl ChatState {
    /// Initialize from a backend session snapshot.
    pub fn new(session: Session) -> Self {
        let status = if session.is_processing {
            RunState::Processing
        } else {
            RunState::Idle
        };
        Self {
            session,
            status,
            permission: None,
            last_error: None,
        }
    }
    /// Insert an optimistic user message. Phase-zero blocks concurrent sends.
    pub fn begin_send(&mut self, text: String, id: String) -> bool {
        if self.session.is_processing
            || text.trim().is_empty()
            || matches!(
                self.status,
                RunState::Processing | RunState::WaitingForPermission
            )
        {
            return false;
        }
        self.session.messages.push(Message {
            id,
            role: "user".into(),
            content: text,
            is_pending: true,
            ..Message::default()
        });
        self.status = RunState::Processing;
        self.session.is_processing = true;
        self.last_error = None;
        true
    }
    /// Roll back only the optimistic message for the failed operation.
    pub fn fail_send(&mut self, id: &str, error: String) {
        self.session
            .messages
            .retain(|m| m.id != id || !m.is_pending);
        self.last_error = Some(error);
        self.status = RunState::Idle;
        self.session.is_processing = false;
    }
    /// Confirm a successful explicit permission response.
    pub fn permission_responded(&mut self, id: &str) {
        if self.permission.as_ref().is_some_and(|r| r.request_id == id) {
            self.permission = None;
            self.status = RunState::Processing;
        }
    }
    /// Apply one actual backend event. Foreign sessions and unknown event kinds are ignored.
    pub fn apply(&mut self, event: &SessionEvent) {
        if event.session_id != self.session.id {
            return;
        }
        let v = &event.payload;
        let text = |key: &str| {
            v.get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        match event.kind.as_str() {
            "user_message" => {
                if let Ok(mut message) = serde_json::from_value::<Message>(v["message"].clone()) {
                    message.is_pending = false;
                    let optimistic = v.get("optimisticMessageId").and_then(Value::as_str);
                    if let Some(index) = self
                        .session
                        .messages
                        .iter()
                        .position(|m| Some(m.id.as_str()) == optimistic || m.id == message.id)
                    {
                        self.session.messages[index] = message;
                    } else {
                        self.session.messages.push(message);
                    }
                    self.status = RunState::Processing;
                    self.session.is_processing = true;
                }
            }
            "text_delta" if self.session.is_processing => {
                let turn = v.get("turnId").and_then(Value::as_str);
                let index = self.session.messages.iter().rposition(|m| {
                    m.role == "assistant" && m.is_streaming && m.turn_id.as_deref() == turn
                });
                if let Some(i) = index {
                    self.session.messages[i].content.push_str(&text("delta"));
                } else {
                    self.session.messages.push(Message {
                        id: format!("stream:{}:{}", self.session.id, self.session.messages.len()),
                        role: "assistant".into(),
                        content: text("delta"),
                        turn_id: turn.map(str::to_owned),
                        is_streaming: true,
                        ..Message::default()
                    });
                }
            }
            "text_complete" if self.session.is_processing => {
                let turn = v.get("turnId").and_then(Value::as_str);
                let index = self.session.messages.iter().rposition(|m| {
                    m.role == "assistant" && m.is_streaming && m.turn_id.as_deref() == turn
                });
                let message = Message {
                    id: v
                        .get("messageId")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .unwrap_or_else(|| {
                            format!("answer:{}:{}", self.session.id, self.session.messages.len())
                        }),
                    role: "assistant".into(),
                    content: text("text"),
                    turn_id: turn.map(str::to_owned),
                    is_intermediate: v["isIntermediate"].as_bool().unwrap_or(false),
                    ..Message::default()
                };
                if let Some(i) = index {
                    self.session.messages[i] = message;
                } else if let Some(i) = self
                    .session
                    .messages
                    .iter()
                    .position(|m| m.id == message.id)
                {
                    self.session.messages[i] = message;
                } else {
                    self.session.messages.push(message);
                }
            }
            "text_discard" => {
                let turn = text("turnId");
                self.session.messages.retain(|m| {
                    !(m.role == "assistant"
                        && m.is_streaming
                        && m.turn_id.as_deref() == Some(&turn))
                });
            }
            "tool_start" if self.session.is_processing => {
                let id = text("toolUseId");
                if !self
                    .session
                    .messages
                    .iter()
                    .any(|m| m.tool_use_id.as_deref() == Some(&id))
                {
                    self.session.messages.push(Message {
                        id: format!("tool:{id}"),
                        role: "tool".into(),
                        tool_name: Some(text("toolName")),
                        tool_use_id: Some(id),
                        tool_input: Some(v["toolInput"].clone()),
                        tool_status: Some("executing".into()),
                        parent_tool_use_id: v
                            .get("parentToolUseId")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        ..Message::default()
                    });
                }
            }
            "tool_result" => {
                let id = text("toolUseId");
                if let Some(m) = self
                    .session
                    .messages
                    .iter_mut()
                    .find(|m| m.tool_use_id.as_deref() == Some(&id))
                {
                    m.tool_result = Some(text("result"));
                    m.is_error = v["isError"].as_bool().unwrap_or(false);
                    m.tool_status = Some(if m.is_error { "error" } else { "completed" }.into());
                }
            }
            "permission_request" => {
                if let Ok(request) = serde_json::from_value(v["request"].clone()) {
                    self.permission = Some(request);
                    self.status = RunState::WaitingForPermission;
                }
            }
            "complete" | "interrupted" => {
                self.session.is_processing = false;
                self.status = if event.kind == "interrupted" {
                    RunState::Interrupted
                } else if self.last_error.is_some() {
                    RunState::Failed
                } else {
                    RunState::Idle
                };
                self.permission = None;
                for m in &mut self.session.messages {
                    m.is_streaming = false;
                    if m.tool_status.as_deref() == Some("executing") {
                        m.tool_status = Some("interrupted".into());
                    }
                }
            }
            "error" | "typed_error" => {
                self.last_error = Some(if v["error"].is_string() {
                    text("error")
                } else {
                    v["error"]["message"]
                        .as_str()
                        .unwrap_or("Agent error")
                        .into()
                });
                self.status = RunState::Failed;
            }
            "title_generated" | "name_changed" => {
                self.session.name = Some(if event.kind == "title_generated" {
                    text("title")
                } else {
                    text("name")
                });
            }
            _ => {}
        }
    }
}
