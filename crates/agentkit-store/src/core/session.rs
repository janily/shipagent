//! Cache and routing policy around the pure formal reducer; no IO or UI dependency.
use super::{
    reducer::{self, Context, Effect, ReducerError, SessionState},
    turns::*,
};
use agentkit_protocol::{DecodedSessionEvent, generated as g};
use std::collections::HashMap;

#[derive(Clone, Debug)]
/// Typed view updates emitted after reduction.
pub enum SessionStoreEvent {
    /// Append text without regrouping history.
    StreamDelta {
        /// Backing message ID.
        message_id: String,
        /// Appended suffix.
        delta: String,
    },
    /// Reconcile a changed card suffix.
    TurnsChanged {
        /// First changed card.
        from_turn: usize,
    },
}
#[derive(Default, Debug)]
/// Effects and view changes produced by an event or merged batch.
pub struct BatchUpdate {
    /// Ordered UI invalidations for this update.
    pub events: Vec<SessionStoreEvent>,
    /// Deferred controller effects.
    pub effects: Vec<Effect>,
    /// Whether consumers need one notification.
    pub changed: bool,
}
#[derive(Clone, Debug)]
/// Per-session optimistic submission awaiting acknowledgement.
pub struct PendingSend {
    /// Stable optimistic identity.
    pub id: String,
    /// Submitted or restored text.
    pub text: String,
    /// Submitted persisted attachment references.
    pub attachments: Vec<g::StoredAttachment>,
    was_processing: bool,
    terminal_epoch: u64,
    draft_revision: u64,
}
#[derive(Debug)]
/// Mutable indexed cache around formal reducer and turns.
pub struct SessionModel {
    /// Canonical generated session and transient stream.
    pub state: SessionState,

    /// Cached render cards; updated incrementally.
    pub turns: Vec<Turn>,

    /// Ordered pending permission requests.
    pub permissions: Vec<g::PermissionRequest>,

    /// Session-local composer contents.
    pub draft: g::SessionDraft,

    /// Last recoverable operation error.
    pub last_error: Option<String>,
    /// Restorable persisted attachment references owned by this session.
    pub draft_attachments: Vec<g::StoredAttachment>,

    /// Number of structural suffix regroup operations.
    pub regroup_count: u64,

    /// Number of in-place streaming updates.
    pub delta_count: u64,
    streams: HashMap<String, usize>,
    last_stream: Option<usize>,
    locations: HashMap<String, (usize, Option<usize>)>,
    turn_ends: Vec<i64>,
    pending: HashMap<String, PendingSend>,
    authoritative_processing: bool,
    draft_revision: u64,
    terminal_epoch: u64,
}
impl SessionModel {
    /// Build an indexed cache from an authoritative generated snapshot.
    pub fn new(session: g::Session) -> Self {
        let processing = session.is_processing;
        let mut this = Self {
            state: SessionState {
                session,
                streaming: None,
            },
            turns: Vec::new(),
            permissions: Vec::new(),
            draft: g::SessionDraft {
                text: String::new(),
                attachments: None,
            },
            last_error: None,
            draft_attachments: Vec::new(),
            regroup_count: 0,
            delta_count: 0,
            streams: HashMap::new(),
            last_stream: None,
            locations: HashMap::new(),
            turn_ends: Vec::new(),
            pending: HashMap::new(),
            authoritative_processing: processing,
            draft_revision: 0,
            terminal_epoch: 0,
        };
        this.regroup(0);
        this
    }
    /// Locate a visible message's cached card without scanning history.
    pub fn turn_index(&self, id: &str) -> Option<usize> {
        self.locations.get(id).map(|(turn, _)| *turn)
    }
    /// Whether work or a permission decision is outstanding.
    pub fn busy(&self) -> bool {
        self.state.session.is_processing || !self.permissions.is_empty()
    }
    /// Insert a pending user message before RPC; preserve queue and attachment state.
    pub fn begin_send(
        &mut self,
        text: String,
        attachments: Vec<g::StoredAttachment>,
        ctx: &Context,
    ) -> Option<PendingSend> {
        if (text.trim().is_empty() && attachments.is_empty())
            || ctx.message_id.is_empty()
            || self
                .state
                .session
                .messages
                .iter()
                .any(|m| m.id == ctx.message_id)
        {
            return None;
        }
        if self.draft_attachments.is_empty() && self.draft.text == text {
            self.draft_attachments = attachments.clone();
        }
        let pending = PendingSend {
            id: ctx.message_id.clone(),
            text: text.clone(),
            attachments: attachments.clone(),
            was_processing: self.state.session.is_processing,
            terminal_epoch: self.terminal_epoch,
            draft_revision: self.draft_revision,
        };
        let mut msg = reducer::message(ctx, g::MessageRole::User, text);
        msg.is_pending = Some(true);
        msg.is_queued = Some(pending.was_processing);
        if !attachments.is_empty() {
            msg.attachments = Some(attachments);
        }
        self.pending.insert(pending.id.clone(), pending.clone());
        self.state.session.messages.push(msg);
        self.state.session.is_processing = true;
        self.last_error = None;
        self.regroup(self.turns.len().saturating_sub(1));
        Some(pending)
    }
    /// Commit acknowledgement or roll back a still-pending optimistic message.
    pub fn finish_send(&mut self, id: &str, accepted: bool) -> Option<PendingSend> {
        let clear_draft = self.draft_is_current(id);
        let pending = self.pending.remove(id)?;
        let accepted = accepted || self.send_confirmed(id);
        if accepted && self.terminal_epoch == pending.terminal_epoch {
            self.authoritative_processing = true;
        }
        if accepted && clear_draft {
            self.draft.text.clear();
            self.draft_attachments.clear();
        }
        if !accepted {
            // An accepted user_message may race a lost acknowledgement: never remove canonical work.
            if let Some(index) = self
                .state
                .session
                .messages
                .iter()
                .position(|m| m.id == id && m.is_pending == Some(true))
            {
                self.state.session.messages.remove(index);
                if self.pending.is_empty() {
                    self.state.session.is_processing = self.authoritative_processing;
                }
                if self.draft.text.is_empty() {
                    self.draft.text = pending.text.clone();
                }
                if self.draft.text == pending.text && self.draft_attachments.is_empty() {
                    self.draft_attachments = pending.attachments.clone();
                }
                self.regroup(0);
            }
        }
        Some(pending)
    }
    /// Set controller-owned draft text and advance its edit generation.
    pub fn set_draft(&mut self, text: String) {
        if self.draft.text != text {
            self.draft.text = text;
            self.draft_revision = self.draft_revision.wrapping_add(1);
        }
    }
    /// Set local attachment references without allowing an older acknowledgement to consume them.
    pub fn set_draft_attachments(&mut self, attachments: Vec<g::StoredAttachment>) {
        if !self
            .draft_attachments
            .iter()
            .map(|a| &a.id)
            .eq(attachments.iter().map(|a| &a.id))
        {
            self.draft_attachments = attachments;
            self.draft_revision = self.draft_revision.wrapping_add(1);
        }
    }
    /// Whether an acknowledgement still owns the current draft, including restored queued input.
    pub fn draft_is_current(&self, id: &str) -> bool {
        self.pending.get(id).is_some_and(|p| {
            p.draft_revision == self.draft_revision
                && self.draft.text == p.text
                && self
                    .draft_attachments
                    .iter()
                    .map(|a| &a.id)
                    .eq(p.attachments.iter().map(|a| &a.id))
        })
    }
    /// Whether a canonical backend echo already accepted this optimistic identity.
    pub fn send_confirmed(&self, id: &str) -> bool {
        self.state
            .session
            .messages
            .iter()
            .any(|m| m.id == id && m.is_pending == Some(false))
    }
    /// Remove only the acknowledged permission request.
    pub fn permission_responded(&mut self, id: &str) {
        self.permissions.retain(|p| p.request_id != id);
    }
    /// Apply one validated event; deltas bypass immutable reducer cloning and grouping.
    pub fn apply(
        &mut self,
        event: &DecodedSessionEvent,
        ctx: &Context,
    ) -> Result<BatchUpdate, ReducerError> {
        let Some(event) = event.known() else {
            return Ok(BatchUpdate::default());
        };
        if reducer::event_session(event) != self.state.session.id {
            return Ok(BatchUpdate::default());
        }
        if ctx.message_id.is_empty() {
            return Err(ReducerError::EmptyMessageId);
        }
        if let g::SessionEvent::TextDeltaEvent(e) = event {
            self.state.session.is_processing = true;
            self.authoritative_processing = true;
            if let Some(Turn::Assistant(t)) = self.turns.last_mut()
                && can_join(t)
            {
                t.is_complete = false;
                if t.response
                    .as_ref()
                    .and_then(|r| r.message_id.as_ref())
                    .is_some_and(|id| t.activities.iter().any(|a| &a.id == id))
                {
                    t.response = None;
                }
            }

            let index = e
                .turn_id
                .as_ref()
                .filter(|s| !s.is_empty())
                .and_then(|id| self.streams.get(id).copied())
                .or(self.last_stream);
            let created = index.is_none();
            let out_of_order =
                created && self.turn_ends.last().is_some_and(|time| ctx.now_ms < *time);
            let index = index.unwrap_or(self.state.session.messages.len());
            reducer::delta_at(
                &mut self.state,
                e,
                ctx,
                if created { None } else { Some(index) },
            );
            let message = &self.state.session.messages[index];
            let id = message.id.clone();
            if created {
                let ti = if matches!(self.turns.last(),Some(Turn::Assistant(t)) if can_join(t)) {
                    self.turns.len() - 1
                } else {
                    self.turns.push(Turn::Assistant(AssistantTurn {
                        kind: AssistantTurnKind::Assistant,
                        turn_id: e
                            .turn_id
                            .as_ref()
                            .filter(|s| !s.is_empty())
                            .unwrap_or(&id)
                            .clone(),
                        activities: Vec::new(),
                        response: None,
                        intent: None,
                        is_streaming: true,
                        is_complete: false,
                        timestamp: message.timestamp,
                        todos: None,
                    }));
                    self.turn_ends.push(message.timestamp);
                    self.turns.len() - 1
                };
                if let Turn::Assistant(t) = &mut self.turns[ti] {
                    let mut a = Activity::new(
                        id.clone(),
                        ActivityType::Intermediate,
                        ActivityStatus::Running,
                        message.timestamp,
                    );
                    a.content = Some(e.delta.clone());
                    a.depth = Some(0);
                    self.locations
                        .insert(id.clone(), (ti, Some(t.activities.len())));
                    t.activities.push(a);
                }
                if let Some(turn) = message.turn_id.as_ref().filter(|s| !s.is_empty()) {
                    self.streams.entry(turn.clone()).or_insert(index);
                }
                self.last_stream = Some(index);
            } else if let Some((ti, ai)) = self.locations.get(&id).copied()
                && let Turn::Assistant(t) = &mut self.turns[ti]
            {
                if let Some(ai) = ai {
                    t.activities[ai]
                        .content
                        .get_or_insert_default()
                        .push_str(&e.delta);
                    if let Some(r) = &mut t.response
                        && r.message_id.as_deref() == Some(id.as_str())
                    {
                        r.text.push_str(&e.delta);
                    }
                } else if let Some(r) = &mut t.response {
                    r.text.push_str(&e.delta);
                }
            }
            self.delta_count += 1;
            if out_of_order {
                let from = self
                    .turn_ends
                    .iter()
                    .position(|time| *time >= ctx.now_ms)
                    .unwrap_or(0);
                self.regroup(from);
                return Ok(BatchUpdate {
                    events: vec![SessionStoreEvent::TurnsChanged { from_turn: from }],
                    effects: Vec::new(),
                    changed: true,
                });
            }
            return Ok(BatchUpdate {
                events: vec![SessionStoreEvent::StreamDelta {
                    message_id: id,
                    delta: e.delta.clone(),
                }],
                effects: vec![],
                changed: true,
            });
        }
        let next = reducer::process(&self.state, event, ctx)?;
        let mut timestamp = ctx.now_ms;
        let mut changed = false;
        for (old, new) in self
            .state
            .session
            .messages
            .iter()
            .map(Some)
            .chain(std::iter::repeat(None))
            .zip(
                next.state
                    .session
                    .messages
                    .iter()
                    .map(Some)
                    .chain(std::iter::repeat(None)),
            )
            .take(
                self.state
                    .session
                    .messages
                    .len()
                    .max(next.state.session.messages.len()),
            )
        {
            if serde_json::to_value(old).ok() != serde_json::to_value(new).ok() {
                changed = true;
                for m in [old, new].into_iter().flatten() {
                    timestamp = timestamp.min(m.timestamp);
                }
            }
        }
        self.state = next.state;
        match event {
            g::SessionEvent::ToolStartEvent(_) | g::SessionEvent::UserMessageEvent(_) => {
                self.authoritative_processing = true;
                self.state.session.is_processing = true;
            }
            g::SessionEvent::CompleteEvent(_)
            | g::SessionEvent::InterruptedEvent(_)
            | g::SessionEvent::ErrorEvent(_)
            | g::SessionEvent::TypedErrorEvent(_) => {
                self.authoritative_processing = false;
                self.terminal_epoch = self.terminal_epoch.wrapping_add(1);
            }
            _ => {}
        }
        for effect in &next.effects {
            match effect {
                Effect::PermissionRequest { request } => {
                    self.permissions
                        .retain(|p| p.request_id != request.request_id);
                    self.permissions.push(request.clone());
                }
                Effect::RestoreInput { text } => {
                    self.draft_revision = self.draft_revision.wrapping_add(1);
                    if self.draft.text.is_empty() {
                        self.draft.text = text.clone();
                    }
                }
                Effect::PermissionModeChanged {
                    permission_mode, ..
                } => self.state.session.permission_mode = Some(*permission_mode),
                Effect::ToastError { message } => self.last_error = Some(message.clone()),
                _ => {}
            }
        }
        if matches!(
            event,
            g::SessionEvent::CompleteEvent(_) | g::SessionEvent::InterruptedEvent(_)
        ) {
            self.permissions.clear();
        }
        let mut update = BatchUpdate {
            events: Vec::new(),
            effects: next.effects,
            changed: true,
        };
        if changed
            || matches!(
                event,
                g::SessionEvent::CompleteEvent(_)
                    | g::SessionEvent::InterruptedEvent(_)
                    | g::SessionEvent::UserMessageEvent(_)
            )
        {
            let mut from = self
                .turn_ends
                .iter()
                .position(|end| *end >= timestamp)
                .unwrap_or(self.turns.len());
            if matches!(self.turns.last(),Some(Turn::Assistant(t)) if can_join(t)) {
                from = from.min(self.turns.len() - 1);
            }
            self.regroup(from);
            update
                .events
                .push(SessionStoreEvent::TurnsChanged { from_turn: from });
        }
        Ok(update)
    }
    fn regroup(&mut self, from: usize) {
        let from = from.min(self.turns.len());
        let prefix: std::collections::HashSet<_> = self
            .locations
            .iter()
            .filter(|(_, (ti, _))| *ti < from)
            .map(|(id, _)| id.as_str())
            .collect();
        let tail: Vec<_> = self
            .state
            .session
            .messages
            .iter()
            .filter(|m| !prefix.contains(m.id.as_str()))
            .cloned()
            .collect();
        let next = group_messages_by_turn(
            &tail,
            GroupTurnsOptions {
                is_session_processing: Some(self.state.session.is_processing),
            },
        );
        self.turns.truncate(from);
        self.turns.extend(next);
        self.regroup_count += 1;
        self.reindex();
    }
    fn reindex(&mut self) {
        self.streams.clear();
        self.last_stream = None;
        self.locations.clear();
        self.turn_ends = vec![i64::MIN; self.turns.len()];
        let times: HashMap<_, _> = self
            .state
            .session
            .messages
            .iter()
            .enumerate()
            .map(|(i, m)| {
                if m.role == g::MessageRole::Assistant && m.is_streaming == Some(true) {
                    self.last_stream = Some(i);
                    if let Some(id) = m.turn_id.as_ref().filter(|s| !s.is_empty()) {
                        self.streams.entry(id.clone()).or_insert(i);
                    }
                }
                (m.id.as_str(), m.timestamp)
            })
            .collect();
        for (ti, t) in self.turns.iter().enumerate() {
            let mut add = |id: &String, ai| {
                self.locations.insert(id.clone(), (ti, ai));
                self.turn_ends[ti] =
                    self.turn_ends[ti].max(*times.get(id.as_str()).unwrap_or(&i64::MIN));
            };
            match t {
                Turn::User(t) => add(&t.message.id, None),
                Turn::System(t) => add(&t.message.id, None),
                Turn::Assistant(t) => {
                    for (ai, a) in t.activities.iter().enumerate() {
                        add(&a.id, Some(ai));
                    }
                    if let Some(r) = &t.response
                        && let Some(id) = &r.message_id
                        && !self.locations.contains_key(id)
                    {
                        self.locations.insert(id.clone(), (ti, None));
                        self.turn_ends[ti] =
                            self.turn_ends[ti].max(*times.get(id.as_str()).unwrap_or(&i64::MIN));
                    }
                }
            }
        }
    }
}

#[derive(Debug, Default)]
/// Summary policy independent of whether a session is open.
pub struct SessionSummaries {
    /// Backend session summaries.
    pub sessions: Vec<g::Session>,
    /// Pending request IDs by session.
    pub awaiting: HashMap<String, Vec<String>>,
    /// Sessions with submitted plans.
    pub plans: std::collections::HashSet<String>,
    /// A create or unknown-session event requires an authoritative list refresh.
    pub refresh_required: bool,
}
impl SessionSummaries {
    /// Apply one validated event; deltas bypass immutable reducer cloning and grouping.
    pub fn apply(&mut self, event: &DecodedSessionEvent) -> bool {
        let Some(e) = event.known() else { return false };
        let id = reducer::event_session(e);
        if matches!(e, g::SessionEvent::SessionCreatedEvent(_)) {
            if self.sessions.iter().any(|s| s.id == id) {
                return false;
            }
            self.refresh_required = true;
            return true;
        }
        if matches!(e, g::SessionEvent::SessionDeletedEvent(_)) {
            self.sessions.retain(|s| s.id != id);
            self.awaiting.remove(id);
            self.plans.remove(id);
            return true;
        }
        let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) else {
            self.refresh_required = true;
            return true;
        };
        use g::SessionEvent as E;
        match e {
            E::UserMessageEvent(_) | E::TextDeltaEvent(_) | E::ToolStartEvent(_) => {
                s.is_processing = true
            }
            E::CompleteEvent(_)
            | E::InterruptedEvent(_)
            | E::ErrorEvent(_)
            | E::TypedErrorEvent(_) => {
                s.is_processing = false;
                self.awaiting.remove(id);
            }
            E::PermissionRequestEvent(e) => {
                let list = self.awaiting.entry(id.into()).or_default();
                if !list.contains(&e.request.request_id) {
                    list.push(e.request.request_id.clone());
                }
            }
            E::PlanSubmittedEvent(_) => {
                self.plans.insert(id.into());
            }
            E::TitleGeneratedEvent(e) => s.name = Some(e.title.clone()),
            E::NameChangedEvent(e) => s.name = e.name.clone(),
            E::SessionFlaggedEvent(_) => s.is_flagged = Some(true),
            E::SessionUnflaggedEvent(_) => s.is_flagged = Some(false),
            E::SessionArchivedEvent(_) => s.is_archived = Some(true),
            E::SessionUnarchivedEvent(_) => s.is_archived = Some(false),
            _ => return false,
        }
        true
    }
    /// Remove only the acknowledged permission request.
    pub fn permission_responded(&mut self, session: &str, request: &str) {
        if let Some(list) = self.awaiting.get_mut(session) {
            list.retain(|id| id != request);
            if list.is_empty() {
                self.awaiting.remove(session);
            }
        }
    }
}
/// Maximum events consumed by a single window wakeup.
pub const MAX_BATCH: usize = 256;

fn can_join(t: &AssistantTurn) -> bool {
    !t.is_complete
        || (!t.activities.iter().any(|a| a.kind == ActivityType::Plan)
            && (t.response.is_none()
                || t.response
                    .as_ref()
                    .and_then(|r| r.message_id.as_ref())
                    .is_some_and(|id| t.activities.iter().any(|a| &a.id == id))))
}
