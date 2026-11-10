//! GPUI wrappers around pure caches. RPC and filesystem work belongs to controllers.
/// Indexed placement of ephemeral conversation annotations.
#[path = "entities_annotations.rs"]
pub mod annotations;
/// Ephemeral native approval leases and acknowledged annotations.
#[path = "entities_permissions.rs"]
pub mod permissions;
/// Native plan presentation state.
#[path = "entities_plans.rs"]
pub mod plans;
use crate::core::{
    reducer::{Context as ReductionContext, Effect},
    session::{SessionModel, SessionStoreEvent, SessionSummaries},
};
use agentkit_protocol::{DecodedSessionEvent, generated as g};
use gpui_kit::{Context, EventEmitter};

/// Global application context shared by workbench stores.
#[derive(Default)]
pub struct AppState {
    /// Whether the backend transport is usable.
    pub connected: bool,
    /// Authenticated backend version, if known.
    pub server_version: Option<String>,
    /// Registered backend RPC channels.
    pub registered_channels: Vec<String>,
    /// Configured UI language preference.
    pub language: String,
    /// Configured theme preference.
    pub theme: String,
    /// Count of sessions marked unread.
    pub unread: f64,
    /// Observed running background tasks across open session caches.
    pub background_tasks: usize,
    /// Safe connection catalog, including authentication availability.
    pub connections: Vec<g::LlmConnectionWithStatus>,
    /// Global default reasoning for sessions without an override.
    pub default_thinking: Option<g::ThinkingLevel>,
}
/// Window-local workspace context; no controller or RPC handles are retained.
#[derive(Default)]
pub struct WorkspaceStore {
    /// Authoritative registered workspace catalog.
    pub catalog: Vec<g::Workspace>,
    /// Workspace identity.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Authoritative workspace metadata.
    pub workspace: Option<g::Workspace>,
    /// Workspace configuration snapshot.
    pub settings: Option<g::WorkspaceSettings>,
    /// Effective directory used for the project skill cache; None means workspace-only.
    pub skills_directory: Option<String>,
    /// Loaded skill metadata.
    pub skills: Vec<g::LoadedSkill>,
}
/// Window-local list and its approval/plan badges.
#[derive(Default)]
pub struct SessionListStore {
    /// Pure generated summary policy.
    pub model: SessionSummaries,
}
impl SessionListStore {
    /// Apply the entire window batch, notifying at most once.
    pub fn apply_batch<'a>(
        &mut self,
        events: impl IntoIterator<Item = &'a DecodedSessionEvent>,
        cx: &mut Context<Self>,
    ) {
        let mut changed = false;
        for event in events {
            let id = event.raw().get("sessionId").and_then(|s| s.as_str());
            if let Some(s) = id.and_then(|id| self.model.sessions.iter_mut().find(|s| s.id == id)) {
                match event.known() {
                    Some(g::SessionEvent::UserMessageEvent(e)) => {
                        s.last_message_at = s.last_message_at.max(e.message.timestamp);
                        s.preview = Some(e.message.content.chars().take(200).collect());
                    }
                    Some(g::SessionEvent::CompleteEvent(e)) => {
                        if let Some(unread) = e.has_unread {
                            s.has_unread = Some(unread);
                        }
                    }
                    Some(
                        g::SessionEvent::TextDeltaEvent(_) | g::SessionEvent::ToolStartEvent(_),
                    ) if s.is_processing => continue,
                    _ => {}
                }
            }
            changed |= self.model.apply(event);
        }
        if changed {
            cx.notify();
        }
    }
    /// Replace an authoritative list while retaining pending approval badges.
    pub fn replace(&mut self, sessions: Vec<g::Session>, cx: &mut Context<Self>) {
        self.model.sessions = sessions
            .into_iter()
            .map(|mut s| {
                s.messages.clear();
                s
            })
            .collect();
        self.model.refresh_required = false;
        cx.notify();
    }
}
/// Per-open-session entity. Views read the canonical cache and subscribe to typed changes.
pub struct SessionStore {
    /// Session-local file payloads and individual warnings.
    pub attachments: crate::core::attachments::Draft,
    /// Plan file cache and current acceptance operation.
    pub plan: plans::PlanState,
    /// Pure mutable session cache.
    pub model: SessionModel,
    /// Controller-owned session configuration RPC in flight; survives cached view switches.
    pub selection_pending: bool,
    /// Controller-owned approval operation and conversation annotations.
    pub approval: permissions::ApprovalState,
    /// Stable row identities for native conversation annotations.
    pub annotations: annotations::AnnotationIndex,
}
impl EventEmitter<SessionStoreEvent> for SessionStore {}
impl SessionStore {
    /// Open an authoritative snapshot once; no RPC occurs here.
    pub fn new(session: g::Session) -> Self {
        let model = SessionModel::new(session);
        let mut annotations = annotations::AnnotationIndex::default();
        annotations.rebuild(&model.turns);
        Self {
            model,
            plan: Default::default(),
            attachments: Default::default(),
            annotations,
            selection_pending: false,
            approval: Default::default(),
        }
    }
    /// Refresh authoritative content in the same Entity, preserving local approvals and drafts.
    /// Controllers serialize this against optimistic sends; backend snapshots omit permission requests.
    pub fn refresh_snapshot(&mut self, session: g::Session, cx: &mut Context<Self>) {
        let mut model = SessionModel::new(session);
        model.draft = self.model.draft.clone();
        model.draft_attachments = self.model.draft_attachments.clone();
        model.last_error = self.model.last_error.clone();
        if model.state.session.is_processing {
            model.permissions = self.model.permissions.clone();
        }
        self.model = model;
        self.refresh_annotations();
        cx.emit(SessionStoreEvent::TurnsChanged { from_turn: 0 });
        cx.notify();
    }
    /// Refresh placement after controller-owned optimistic sends or initial backlog application.
    pub fn refresh_annotations(&mut self) {
        self.annotations.rebuild(&self.model.turns);
    }
    /// Apply a session-specific batch; semantic failures become deferred error effects.
    /// Valid neighbors remain ordered and one Entity notification follows.
    pub fn apply_batch(
        &mut self,
        events: &[(DecodedSessionEvent, ReductionContext)],
        cx: &mut Context<Self>,
    ) -> Vec<Effect> {
        let mut effects = Vec::new();
        let mut changed = false;
        let mut output = Vec::new();
        let mut from = None;
        for (event, context) in events {
            let update = match self.model.apply(event, context) {
                Ok(update) => update,
                Err(_) => {
                    effects.push(Effect::ToastError {
                        message: "ak.event_invalid".into(),
                    });
                    continue;
                }
            };
            changed |= update.changed;
            effects.extend(update.effects);
            for event in update.events {
                match event {
                    SessionStoreEvent::TurnsChanged { from_turn } => {
                        from = Some(from.map_or(from_turn, |old: usize| old.min(from_turn)))
                    }
                    SessionStoreEvent::StreamDelta { message_id, delta } => {
                        if let Some(SessionStoreEvent::StreamDelta {
                            message_id: old,
                            delta: text,
                        }) = output.last_mut()
                            && *old == message_id
                        {
                            text.push_str(&delta);
                        } else {
                            output.push(SessionStoreEvent::StreamDelta { message_id, delta });
                        }
                    }
                }
            }
        }
        if let Some(from_turn) = from {
            self.annotations.rebuild(&self.model.turns);
            cx.emit(SessionStoreEvent::TurnsChanged { from_turn });
        } else {
            for event in output {
                cx.emit(event);
            }
        }
        if changed {
            cx.notify();
        }
        effects
    }
}
