//! Session-scoped native input. Emits intent packets; owns no backend handles.
mod keyboard;
mod menus;
mod policy;
mod render;
use crate::{WorkbenchIntent, t};
use agentkit_protocol::generated as g;
use agentkit_store::entities::SessionStore;
use gpui_kit::component::input::{InputEvent, TextareaState};
use gpui_kit::{prelude::*, *};
pub(crate) use policy::resolved_model;
pub use policy::{ModelChoice, SendKey, model_choices};

pub(crate) struct Config {
    pub store: Option<Entity<SessionStore>>,
    pub ready: bool,
    pub submitting: bool,
    pub loading: bool,
    pub choices: Vec<ModelChoice>,
    pub model: Option<String>,
    pub connection: Option<String>,
    pub thinking: g::ThinkingLevel,
}
/// Independent native message editor and session toolbar.
pub struct Composer {
    pub(super) escape_stops: bool,
    input: Entity<TextareaState>,
    store: Option<Entity<SessionStore>>,
    ready: bool,
    submitting: bool,
    loading: bool,
    changing: bool,
    choices: Vec<ModelChoice>,
    fallback_model: Option<String>,
    default_connection: Option<String>,
    default_thinking: g::ThinkingLevel,
    send_key: SendKey,
    status: (
        bool,
        bool,
        Option<g::PermissionMode>,
        Option<g::ThinkingLevel>,
    ),
    _subscription: Subscription,
}
impl EventEmitter<WorkbenchIntent> for Composer {}
impl Composer {
    /// Create immediately with an empty, disabled editor.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 12)
                .submit_on_enter(true)
        });
        let sub = cx.subscribe_in(&input, window, |this, _, event, _, cx| {
            if let InputEvent::Change = event {
                if let Some(store) = &this.store {
                    cx.emit(WorkbenchIntent::DraftChanged {
                        session_id: store.read(cx).model.state.session.id.clone(),
                        text: this.input.read(cx).value().to_string(),
                    });
                }
                cx.notify();
            }
        });
        Self {
            escape_stops: true,
            input,
            store: None,
            ready: false,
            submitting: false,
            loading: false,
            changing: false,
            choices: Vec::new(),
            fallback_model: None,
            default_connection: None,
            default_thinking: g::ThinkingLevel::Medium,
            send_key: SendKey::Enter,
            status: (false, false, None, None),
            _subscription: sub,
        }
    }
    /// Enable legacy TextInput Escape cancellation only when the effective keymap retains it.
    pub fn set_escape_stops(&mut self, enabled: bool) {
        self.escape_stops = enabled;
    }
    /// Native text state, shared with the containing workbench's draft bridge.
    pub fn input(&self) -> &Entity<TextareaState> {
        &self.input
    }
    pub(crate) fn bind(&mut self, store: Option<Entity<SessionStore>>) {
        if self.store.as_ref().map(Entity::entity_id) != store.as_ref().map(Entity::entity_id) {
            self.changing = false;
        }
        self.store = store;
    }
    pub(crate) fn configure(&mut self, config: Config, cx: &mut Context<Self>) {
        let pending = config
            .store
            .as_ref()
            .is_some_and(|s| s.read(cx).selection_pending);
        let status = config
            .store
            .as_ref()
            .map(|s| {
                let m = &s.read(cx).model;
                (
                    m.state.session.is_processing,
                    !m.permissions.is_empty(),
                    m.state.session.permission_mode,
                    m.state.session.thinking_level,
                )
            })
            .unwrap_or((false, false, None, None));
        let changed = self.store.as_ref().map(Entity::entity_id)
            != config.store.as_ref().map(Entity::entity_id)
            || self.ready != config.ready
            || self.submitting != config.submitting
            || self.loading != config.loading
            || self.choices != config.choices
            || self.fallback_model != config.model
            || self.default_connection != config.connection
            || self.default_thinking != config.thinking
            || self.changing != pending
            || self.status != status;
        self.bind(config.store);
        self.changing = pending;
        self.status = status;
        self.ready = config.ready;
        self.submitting = config.submitting;
        self.loading = config.loading;
        self.choices = config.choices;
        self.fallback_model = config.model;
        self.default_connection = config.connection;
        self.default_thinking = config.thinking;
        if changed {
            cx.notify();
        }
    }
    /// Restore the persisted local send-key choice without writing a new preference.
    pub fn set_send_key(&mut self, key: SendKey, cx: &mut Context<Self>) {
        self.send_key = key;
        self.input.update(cx, |input, cx| {
            input.set_submit_on_enter(key == SendKey::Enter, cx)
        });
        cx.notify();
    }
    /// Finish a session configuration operation; changes are confirmed by the controller.
    pub fn selection_finished(&mut self, cx: &mut Context<Self>) {
        self.changing = false;
        cx.notify();
    }
    fn session<'a>(&self, cx: &'a App) -> Option<&'a g::Session> {
        self.store.as_ref().map(|s| &s.read(cx).model.state.session)
    }
    fn selectable(&self) -> bool {
        self.ready && self.store.is_some() && !self.loading
    }
    fn enabled(&self) -> bool {
        self.ready && self.store.is_some() && !self.loading && self.fallback_model.is_some()
    }
    fn composing(&self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.input.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        })
    }
    pub(crate) fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.input.read(cx).value().to_string();
        if self.enabled()
            && !self.submitting
            && !self.changing
            && !text.trim().is_empty()
            && !self.composing(window, cx)
        {
            self.submitting = true;
            cx.emit(WorkbenchIntent::Submit(text));
            cx.notify();
        }
    }
    fn stop(&mut self, cx: &mut Context<Self>) {
        if self.ready && self.session(cx).is_some_and(|s| s.is_processing) {
            cx.emit(WorkbenchIntent::Cancel);
        }
    }
}

#[cfg(feature = "e2e")]
impl Composer {
    pub(crate) fn e2e_ready(&self) -> bool {
        self.enabled() && !self.submitting && !self.changing
    }
}
