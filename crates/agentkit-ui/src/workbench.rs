use crate::t;
#[path = "shell.rs"]
mod shell;
use crate::layout::ShellPreferences;
use agentkit_store::{
    core::session::SessionStoreEvent,
    entities::{AppState, SessionListStore, SessionStore, WorkspaceStore},
};
use gpui_kit::component::resizable::ResizableState;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState, TextareaState},
    message_scroller::MessageScrollerState,
    text::TextViewState,
    *,
};
use gpui_kit::{prelude::*, *};
use secrecy::SecretString;
use std::collections::{HashMap, HashSet};

/// Semantic UI actions, handled only by the app controller.
#[derive(Clone)]
pub enum WorkbenchIntent {
    /// Submit a draft; no credential or RPC channel appears in the view.
    Submit(String),
    /// Session-scoped local draft change; valid even while disconnected.
    DraftChanged {
        /// Owning session ID.
        session_id: String,
        /// Current composer text.
        text: String,
    },
    /// Stop the active round.
    Cancel,
    /// Create a safe empty conversation.
    NewSession,
    /// Open persisted conversation messages.
    OpenSession(String),
    /// Explicit tool permission decision.
    Permission {
        /// Backend request identifier.
        request_id: String,
        /// User granted this invocation.
        allowed: bool,
        /// User requested a session-wide grant.
        always: bool,
    },
    /// Save a user-entered compatible endpoint.
    Configure {
        /// OpenAI-compatible API root.
        base_url: String,
        /// User-entered model identifier.
        model: String,
        /// Memory-only credential handed to the backend.
        credential: Option<SecretString>,
    },
    /// Retry backend startup or connection after failure.
    Reconnect,
    /// Persist a user shell layout change locally.
    LayoutChanged(ShellPreferences),
    /// Cycle system/light/dark appearance in the application layer.
    CycleAppearance,
}
/// Native fixed-layout prototype backed by actual conversation state.
pub struct WorkbenchView {
    pub(crate) app_name: SharedString,
    shell: ShellPreferences,
    panel_sizes: Entity<ResizableState>,
    sheet_kind: Option<bool>,
    sidebar_sheet: bool,
    overlay: Option<Entity<shell::ShellOverlay>>,
    preferences_writable: bool,
    pub(crate) workspace: SharedString,
    pub(crate) model: Option<String>,
    pub(crate) status_key: &'static str,
    pub(crate) ready: bool,
    reload_required: bool,
    pub(crate) loading: bool,
    pub(crate) error: Option<String>,
    pub(crate) state: Option<Entity<SessionStore>>,
    pub(crate) sessions: Option<Entity<SessionListStore>>,
    pub(crate) composer: Entity<TextareaState>,
    pub(crate) scroll: Entity<MessageScrollerState>,
    pub(crate) markdown: HashMap<String, (String, Entity<TextViewState>)>,
    pub(crate) expanded: HashSet<String>,
    pub(crate) setup: bool,
    pub(crate) submitting: bool,
    configuring: bool,
    pub(crate) permission_busy: bool,
    permission_id: Option<String>,
    base_url: Entity<InputState>,
    model_id: Entity<InputState>,
    api_key: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
    session_subscriptions: Vec<Subscription>,
    list_subscription: Option<Subscription>,
    app_state: Option<Entity<AppState>>,
    context_subscriptions: Vec<Subscription>,
}
impl EventEmitter<WorkbenchIntent> for WorkbenchView {}
impl WorkbenchView {
    /// Construct the window immediately, while backend startup happens asynchronously.
    pub fn new(app_name: SharedString, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder(t!("ak.placeholder"))
                .auto_grow(2, 12)
                .submit_on_enter(true)
        });
        let input_subscription =
            cx.subscribe_in(&composer, window, |this, _, event, _, cx| match event {
                InputEvent::PressEnter { shift: false, .. } => this.submit(cx),
                InputEvent::Change => {
                    this.save_draft(cx);
                    cx.notify();
                }
                _ => {}
            });
        Self {
            app_name,
            shell: ShellPreferences::default(),
            panel_sizes: cx.new(|_| ResizableState::default()),
            sheet_kind: None,
            sidebar_sheet: false,
            overlay: None,
            preferences_writable: true,
            workspace: t!("ak.app").into(),
            model: None,
            status_key: "ak.starting",
            ready: false,
            reload_required: false,
            loading: false,
            error: None,
            state: None,
            sessions: None,
            composer,
            scroll: cx.new(|cx| MessageScrollerState::new(0, cx)),
            markdown: HashMap::new(),
            expanded: HashSet::new(),
            setup: false,
            submitting: false,
            configuring: false,
            permission_busy: false,
            permission_id: None,
            base_url: cx
                .new(|cx| InputState::new(window, cx).placeholder("http://127.0.0.1:11434/v1")),
            model_id: cx.new(|cx| InputState::new(window, cx)),
            api_key: cx.new(|cx| InputState::new(window, cx).masked(true)),
            _subscriptions: vec![input_subscription],
            session_subscriptions: Vec::new(),
            list_subscription: None,
            app_state: None,
            context_subscriptions: Vec::new(),
        }
    }
    /// Apply actual bootstrap results; no simulated connected state is used by the app.
    pub fn connected(&mut self, workspace: String, model: Option<String>, cx: &mut Context<Self>) {
        self.workspace = workspace.into();
        self.model = model;
        self.ready = true;
        self.reload_required = false;
        self.status_key = "ak.connected";
        self.setup = self.model.is_none();
        self.error = None;
        cx.notify();
    }
    /// Recover transport availability; stale state requires an explicit reload.
    pub fn transport_recovered(&mut self, full_refresh: bool, cx: &mut Context<Self>) {
        self.reload_required |= full_refresh;
        if self.reload_required {
            self.show_error(t!("ak.reload_required"), true, cx);
        } else {
            self.ready = true;
            self.error = None;
            self.status_key = "ak.connected";
            cx.notify();
        }
    }
    /// Observe global and workspace context without retaining network handles.
    pub fn bind_context(
        &mut self,
        app: Entity<AppState>,
        workspace: Entity<WorkspaceStore>,
        cx: &mut Context<Self>,
    ) {
        self.context_subscriptions.clear();
        self.app_state = Some(app.clone());
        self.context_subscriptions
            .push(cx.observe(&app, |_, _, cx| cx.notify()));
        self.context_subscriptions
            .push(cx.observe(&workspace, |this, workspace, cx| {
                this.workspace = workspace.read(cx).name.clone().into();
                cx.notify();
            }));
    }
    /// Read sidebar summaries from the window's list Entity.
    pub fn bind_list(&mut self, list: Entity<SessionListStore>, cx: &mut Context<Self>) {
        self.list_subscription = Some(cx.observe(&list, |_, _, cx| cx.notify()));
        self.sessions = Some(list);
        cx.notify();
    }
    /// Bind a canonical session Entity; no RPC or conversation snapshots live in the view.
    pub fn bind_session(
        &mut self,
        store: Entity<SessionStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .state
            .as_ref()
            .is_some_and(|s| s.entity_id() == store.entity_id())
        {
            self.loading = false;
            cx.notify();
            return;
        }
        self.session_subscriptions.clear();
        self.markdown.clear();
        self.expanded.clear();
        self.scroll.update(cx, |s, cx| s.reset(0, cx));
        self.error = store.read(cx).model.last_error.clone();
        self.state = Some(store.clone());
        self.reconcile(0, cx);
        let draft = store.read(cx).model.draft.text.clone();
        self.composer
            .update(cx, |input, cx| input.set_value(draft, window, cx));
        self.session_subscriptions.push(cx.subscribe(
            &store,
            |this, store, event, cx| match event {
                SessionStoreEvent::TurnsChanged { from_turn } => this.reconcile(*from_turn, cx),
                SessionStoreEvent::StreamDelta { message_id, delta } => {
                    if let Some((text, entity)) = this.markdown.get_mut(message_id) {
                        text.push_str(delta);
                        entity.update(cx, |state, cx| state.push_str(delta, cx));
                    } else if let Some(message) = store
                        .read(cx)
                        .model
                        .state
                        .session
                        .messages
                        .iter()
                        .find(|m| &m.id == message_id && m.hidden != Some(true))
                    {
                        let content = message.content.clone();
                        let state = cx.new(|cx| TextViewState::markdown(&content, cx));
                        this.markdown.insert(message_id.clone(), (content, state));
                    }
                    this.sync_rows(cx);
                    if let Some(row) = store.read(cx).model.turn_index(message_id) {
                        this.scroll
                            .update(cx, |s, cx| s.remeasure_items(row..row + 1, cx));
                    }
                }
            },
        ));
        self.session_subscriptions
            .push(cx.observe(&store, |this, _, cx| {
                this.sync_rows(cx);
                cx.notify();
            }));
        if self.setup {
            self.base_url
                .update(cx, |input, cx| input.focus(window, cx));
        } else {
            self.composer
                .update(cx, |input, cx| input.focus(window, cx));
        }
        self.loading = false;
        self.submitting = false;
        self.permission_busy = false;
        cx.notify();
    }
    fn sync_rows(&mut self, cx: &mut Context<Self>) {
        let count = self
            .state
            .as_ref()
            .map_or(0, |s| s.read(cx).model.turns.len());
        let old = self.scroll.read(cx).item_count();
        self.scroll.update(cx, |scroll, cx| {
            if count < old {
                scroll.splice(count..old, 0, cx);
            } else if count > old {
                scroll.append(count - old, cx);
            }
        });
        let request = self.state.as_ref().and_then(|s| {
            s.read(cx)
                .model
                .permissions
                .first()
                .map(|r| r.request_id.clone())
        });
        if self.permission_id != request {
            self.permission_busy = false;
            self.permission_id = request;
        }
    }
    fn reconcile(&mut self, from: usize, cx: &mut Context<Self>) {
        let Some(store) = self.state.clone() else {
            return;
        };
        let model = &store.read(cx).model;
        let ids: HashSet<_> = model
            .turns
            .iter()
            .skip(from)
            .flat_map(|t| match t {
                agentkit_store::core::turns::Turn::Assistant(t) => t
                    .activities
                    .iter()
                    .filter(|a| a.kind == agentkit_store::core::turns::ActivityType::Intermediate)
                    .map(|a| a.id.clone())
                    .chain(t.response.iter().filter_map(|r| r.message_id.clone()))
                    .collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .collect();
        let messages: Vec<_> = model
            .state
            .session
            .messages
            .iter()
            .filter(|m| ids.contains(&m.id))
            .map(|m| (m.id.clone(), m.content.clone()))
            .collect();
        let retained: HashSet<_> = model
            .state
            .session
            .messages
            .iter()
            .map(|m| m.id.clone())
            .collect();
        self.markdown.retain(|id, _| retained.contains(id));
        for (id, text) in messages {
            if let Some((previous, entity)) = self.markdown.get_mut(&id) {
                if *previous != text {
                    entity.update(cx, |s, cx| s.set_text(&text, cx));
                    *previous = text;
                }
            } else {
                let entity = cx.new(|cx| TextViewState::markdown(&text, cx));
                self.markdown.insert(id, (text, entity));
            }
        }
        let count = store.read(cx).model.turns.len();
        let old = self.scroll.read(cx).item_count();
        let start = from.min(old).min(count);
        self.scroll.update(cx, |s, cx| {
            if start < old || start < count {
                s.splice(start..old, count - start, cx);
            }
            if start < count {
                s.remeasure_items(start..count, cx);
            }
        });
        self.sync_rows(cx);
    }
    /// Inspect virtual-list state in component and performance sample builds.
    #[cfg(feature = "samples")]
    pub fn scroll_state(&self) -> &Entity<MessageScrollerState> {
        &self.scroll
    }
    /// Inspect rendered Markdown content in component/performance sample builds.
    #[cfg(feature = "samples")]
    pub fn cached_markdown(&self, id: &str) -> Option<&str> {
        self.markdown.get(id).map(|(text, _)| text.as_str())
    }
    /// Detach a deleted session; the backend connection remains available for new sessions.
    pub fn clear_session(&mut self, cx: &mut Context<Self>) {
        self.session_subscriptions.clear();
        self.state = None;
        self.markdown.clear();
        self.expanded.clear();
        self.permission_id = None;
        self.permission_busy = false;
        self.scroll.update(cx, |s, cx| s.reset(0, cx));
        cx.notify();
    }
    /// Current local draft packet, for controller-owned persistence before switching.
    pub fn current_draft(&self, cx: &App) -> Option<(String, String)> {
        self.state.as_ref().map(|store| {
            (
                store.read(cx).model.state.session.id.clone(),
                self.composer.read(cx).value().to_string(),
            )
        })
    }
    /// Emit a draft intent; the view never mutates the shared session Entity.
    pub fn save_draft(&mut self, cx: &mut Context<Self>) {
        if let Some((session_id, text)) = self.current_draft(cx) {
            cx.emit(WorkbenchIntent::DraftChanged { session_id, text });
        }
    }
    /// Restore text without overwriting newer user input.
    pub fn restore_draft(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.composer.read(cx).value().is_empty() {
            self.composer
                .update(cx, |input, cx| input.set_value(text, window, cx));
        }
    }
    /// Preserve draft text until send persistence is acknowledged; do not clear later edits.
    pub fn submission_finished(
        &mut self,
        submitted: &str,
        success: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.submitting = false;
        if success && self.composer.read(cx).value().as_ref() == submitted {
            self.composer
                .update(cx, |input, cx| input.set_value("", window, cx));
        }
        cx.notify();
    }
    /// Clear the credential field once connection setup finishes successfully.
    pub fn configuration_finished(
        &mut self,
        success: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.configuring = false;
        if success {
            self.api_key
                .update(cx, |input, cx| input.set_value("", window, cx));
            self.setup = false;
        }
        cx.notify();
    }
    /// Present a recoverable operation or transport error without discarding history.
    pub fn show_error(&mut self, error: String, disconnected: bool, cx: &mut Context<Self>) {
        self.error = Some(error);
        self.submitting = false;
        self.configuring = false;
        self.permission_busy = false;
        self.loading = false;
        if disconnected {
            self.ready = false;
            self.status_key = "ak.disconnected";
        }
        cx.notify();
    }
    /// Show session loading without replacing its message history with false content.
    pub fn set_loading(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        cx.notify();
    }
    pub(crate) fn busy(&self, cx: &App) -> bool {
        self.state
            .as_ref()
            .is_some_and(|store| store.read(cx).model.busy())
    }
    pub(crate) fn submit(&mut self, cx: &mut Context<Self>) {
        let text = self.composer.read(cx).value().to_string();
        if self.state.is_some()
            && self.ready
            && !self.loading
            && self.model.is_some()
            && !self.submitting
            && !text.trim().is_empty()
        {
            self.error = None;
            self.submitting = true;
            cx.emit(WorkbenchIntent::Submit(text));
            cx.notify();
        }
    }
    fn endpoint_form(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .w_full()
            .max_w(px(620.))
            .gap_4()
            .p_6()
            .child(div().text_lg().font_semibold().child(t!("ak.settings")))
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("ak.setup_hint")),
            )
            .child(t!("ak.url"))
            .child(
                Input::new(&self.base_url)
                    .id("endpoint-url")
                    .aria_label(t!("ak.url")),
            )
            .child(t!("ak.model"))
            .child(
                Input::new(&self.model_id)
                    .id("endpoint-model")
                    .aria_label(t!("ak.model")),
            )
            .child(t!("ak.key"))
            .child(
                Input::new(&self.api_key)
                    .id("endpoint-key")
                    .aria_label(t!("ak.key")),
            )
            .child(
                Button::new("save-endpoint")
                    .primary()
                    .label(t!("ak.save"))
                    .disabled(!self.ready || self.configuring)
                    .on_click(cx.listener(|this, _, _, cx| {
                        let url = this.base_url.read(cx).value().to_string();
                        let model = this.model_id.read(cx).value().to_string();
                        let key = this.api_key.read(cx).value().to_string();
                        if url.trim().is_empty() || model.trim().is_empty() {
                            return;
                        }
                        this.configuring = true;
                        cx.emit(WorkbenchIntent::Configure {
                            base_url: url,
                            model,
                            credential: if key.is_empty() {
                                None
                            } else {
                                Some(key.into())
                            },
                        });
                        cx.notify();
                    })),
            )
            .when(self.model.is_some(), |view| {
                view.child(
                    Button::new("back-chat")
                        .ghost()
                        .label(t!("ak.back"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.setup = false;
                            cx.notify();
                        })),
                )
            })
            .into_any_element()
    }
}
impl Render for WorkbenchView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_shell(window, cx)
    }
}
