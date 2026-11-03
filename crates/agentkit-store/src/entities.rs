//! GPUI wrappers around pure caches. RPC and filesystem work belongs to controllers.
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
}
/// Window-local workspace context; no controller or RPC handles are retained.
#[derive(Default)]
pub struct WorkspaceStore {
    /// Workspace identity.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Authoritative workspace metadata.
    pub workspace: Option<g::Workspace>,
    /// Workspace configuration snapshot.
    pub settings: Option<g::WorkspaceSettings>,
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
    /// Pure mutable session cache.
    pub model: SessionModel,
}
impl EventEmitter<SessionStoreEvent> for SessionStore {}
impl SessionStore {
    /// Open an authoritative snapshot once; no RPC occurs here.
    pub fn new(session: g::Session) -> Self {
        Self {
            model: SessionModel::new(session),
        }
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
