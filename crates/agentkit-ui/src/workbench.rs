use crate::t;
#[path = "shell.rs"]
mod shell;
use crate::layout::ShellPreferences;
use crate::transcript::{ChatIntent, ChatView};
use agentkit_store::entities::{AppState, SessionListStore, SessionStore, WorkspaceStore};
use gpui_kit::component::resizable::ResizableState;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::{Input, InputState},
    message_scroller::MessageScrollerState,
    *,
};
use gpui_kit::{prelude::*, *};
use secrecy::SecretString;

/// Semantic UI actions, handled only by the app controller.
#[derive(Clone)]
pub enum WorkbenchIntent {
    /// Session/workspace navigation bound to stable identities.
    Sidebar(crate::sidebar::SidebarIntent),
    /// Select an authenticated connection and model for this session.
    ModelChanged {
        /// Connection identity.
        connection: String,
        /// Exact model ID.
        model: String,
    },
    /// Session-specific thinking level.
    ThinkingChanged(agentkit_protocol::generated::ThinkingLevel),
    /// Explicit session permission mode choice.
    ModeChanged(agentkit_protocol::generated::PermissionMode),
    /// Persist the client keyboard preference.
    SendKeyChanged(crate::composer::SendKey),
    /// Conversation component intent, resolved against canonical state by the controller.
    Chat(ChatIntent),
    /// Submit a draft; no credential or RPC channel appears in the view.
    Submit(String),
    /// Session-scoped local draft change; valid even while disconnected.
    DraftChanged {
        /// Owning session ID.
        session_id: String,
        /// Current composer text.
        text: String,
    },
    /// Explicitly resend a canonical user message, including its stored attachments.
    ResendMessage(String),
    /// Stop the active round.
    Cancel,
    /// Create a safe empty conversation.
    NewSession,
    /// Open persisted conversation messages.
    OpenSession(String),
    /// Explicit tool permission decision.
    Permission {
        /// Session captured when the user made the decision.
        session_id: String,
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
/// Native application shell hosting the canonical conversation components.
pub struct WorkbenchView {
    pub(crate) sidebar_view: Entity<crate::sidebar::SessionSidebar>,
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
    pub(crate) composer: Entity<crate::composer::Composer>,
    pub(crate) scroll: Entity<MessageScrollerState>,
    pub(crate) chat: Entity<ChatView>,
    pub(crate) setup: bool,
    pub(crate) submitting: bool,
    configuring: bool,
    pub(crate) permission_prompt: Entity<crate::permissions::PermissionPrompt>,
    base_url: Entity<InputState>,
    model_id: Entity<InputState>,
    api_key: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
    session_subscriptions: Vec<Subscription>,
    list_subscription: Option<Subscription>,
    pub(crate) app_state: Option<Entity<AppState>>,
    pub(crate) workspace_state: Option<Entity<WorkspaceStore>>,
    context_subscriptions: Vec<Subscription>,
}
impl EventEmitter<WorkbenchIntent> for WorkbenchView {}
impl WorkbenchView {
    /// Construct the window immediately, while backend startup happens asynchronously.
    pub fn new(app_name: SharedString, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let sidebar_view = cx.new(|cx| crate::sidebar::SessionSidebar::new(window, cx));
        let sidebar_sub = cx.subscribe(&sidebar_view, |_, _, intent, cx| {
            cx.emit(WorkbenchIntent::Sidebar(intent.clone()))
        });
        let composer = cx.new(|cx| crate::composer::Composer::new(window, cx));
        let input_subscription = cx.subscribe(&composer, |this, _, event: &WorkbenchIntent, cx| {
            if matches!(event, WorkbenchIntent::Submit(_)) {
                this.error = None;
                this.submitting = true;
            }
            cx.emit(event.clone());
            cx.notify();
        });
        let permission_prompt = cx.new(crate::permissions::PermissionPrompt::new);
        let permission_subscription = cx
            .subscribe(&permission_prompt, |_, _, event: &WorkbenchIntent, cx| {
                cx.emit(event.clone())
            });
        let chat = cx.new(ChatView::new);
        let scroll = chat.read(cx).scroll_state().clone();
        let chat_subscription = cx.subscribe(&chat, |_, _, intent, cx| {
            cx.emit(WorkbenchIntent::Chat(intent.clone()))
        });
        Self {
            sidebar_view,
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
            scroll,
            chat,
            setup: false,
            submitting: false,
            configuring: false,
            permission_prompt,
            base_url: cx
                .new(|cx| InputState::new(window, cx).placeholder("http://127.0.0.1:11434/v1")),
            model_id: cx.new(|cx| InputState::new(window, cx)),
            api_key: cx.new(|cx| InputState::new(window, cx).masked(true)),
            _subscriptions: vec![
                sidebar_sub,
                input_subscription,
                chat_subscription,
                permission_subscription,
            ],
            session_subscriptions: Vec::new(),
            list_subscription: None,
            app_state: None,
            workspace_state: None,
            context_subscriptions: Vec::new(),
        }
    }
    /// Show the existing local model connection form without changing drafts.
    pub fn show_configuration(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.permission_prompt.update(cx, |p, cx| p.suspend(cx));
        self.setup = true;
        self.base_url
            .update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }
    /// Apply actual bootstrap results; no simulated connected state is used by the app.
    pub fn connected(&mut self, workspace: String, model: Option<String>, cx: &mut Context<Self>) {
        self.workspace = workspace.into();
        self.model = model;
        self.ready = true;
        self.reload_required = false;
        self.status_key = "ak.connected";
        self.setup = self.model.is_none();
        if self.setup {
            self.permission_prompt.update(cx, |p, cx| p.suspend(cx));
        }
        self.error = None;
        cx.notify();
    }
    /// Update the workspace label without altering connection setup state.
    pub fn workspace_changed(&mut self, name: String, cx: &mut Context<Self>) {
        self.workspace = name.into();
        self.loading = false;
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
        self.workspace_state = Some(workspace.clone());
        self.sidebar_view
            .update(cx, |s, cx| s.bind_workspace(workspace.clone(), cx));
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
        self.sidebar_view
            .update(cx, |s, cx| s.bind_list(list.clone(), cx));
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
        self.chat
            .update(cx, |chat, cx| chat.bind(store.clone(), cx));
        self.error = store.read(cx).model.last_error.clone();
        self.state = Some(store.clone());
        self.composer.update(cx, |c, _| c.bind(Some(store.clone())));

        let draft = store.read(cx).model.draft.text.clone();
        self.composer
            .read(cx)
            .input()
            .clone()
            .update(cx, |input, cx| input.set_value(draft, window, cx));
        self.session_subscriptions
            .push(cx.observe(&store, |_, _, cx| {
                cx.notify();
            }));
        if self.setup {
            self.base_url
                .update(cx, |input, cx| input.focus(window, cx));
        } else if !self.sidebar_view.read(cx).focused(window, cx) {
            self.composer
                .read(cx)
                .input()
                .clone()
                .update(cx, |input, cx| input.focus(window, cx));
        }
        self.permission_prompt
            .update(cx, |p, cx| p.bind(Some(store), window, cx));
        self.loading = false;
        self.submitting = false;
        cx.notify();
    }
    /// Inspect native text-input protocol in isolated component tests.
    #[cfg(feature = "samples")]
    pub fn composer_input(&self, cx: &App) -> Entity<gpui_kit::component::input::TextareaState> {
        self.composer.read(cx).input().clone()
    }
    /// Inspect virtual-list state in component and performance sample builds.
    #[cfg(feature = "samples")]
    pub fn scroll_state(&self) -> &Entity<MessageScrollerState> {
        &self.scroll
    }
    /// Inspect rendered Markdown content in component/performance sample builds.
    #[cfg(feature = "samples")]
    pub fn cached_markdown(&self, id: &str, cx: &App) -> Option<String> {
        self.chat.read(cx).cached_markdown(id, cx)
    }
    /// Detach a deleted session; the backend connection remains available for new sessions.
    pub fn clear_session(&mut self, cx: &mut Context<Self>) {
        self.session_subscriptions.clear();
        self.permission_prompt.update(cx, |p, cx| p.suspend(cx));
        self.state = None;
        self.composer.update(cx, |c, _| c.bind(None));
        self.chat.update(cx, |chat, cx| chat.clear(cx));
        self.scroll.update(cx, |s, cx| s.reset(0, cx));
        cx.notify();
    }
    /// Current local draft packet, for controller-owned persistence before switching.
    pub fn current_draft(&self, cx: &App) -> Option<(String, String)> {
        self.state.as_ref().map(|store| {
            (
                store.read(cx).model.state.session.id.clone(),
                self.composer.read(cx).input().read(cx).value().to_string(),
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
        if self.composer.read(cx).input().read(cx).value().is_empty() {
            self.composer
                .read(cx)
                .input()
                .clone()
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
        if success && self.composer.read(cx).input().read(cx).value().as_ref() == submitted {
            self.composer
                .read(cx)
                .input()
                .clone()
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
        self.loading = false;
        if disconnected {
            self.permission_prompt.update(cx, |p, cx| p.suspend(cx));
            self.composer.update(cx, |c, cx| c.selection_finished(cx));
            self.ready = false;
            self.status_key = "ak.disconnected";
        }
        cx.notify();
    }
    /// Show session loading without replacing its message history with false content.
    pub fn set_loading(&mut self, cx: &mut Context<Self>) {
        self.permission_prompt.update(cx, |p, cx| p.suspend(cx));
        self.loading = true;
        cx.notify();
    }
    pub(crate) fn busy(&self, cx: &App) -> bool {
        self.state
            .as_ref()
            .is_some_and(|store| store.read(cx).model.busy())
    }
    /// Report a recoverable OS alert failure without disabling approval controls.
    pub fn notification_unavailable(&mut self, cx: &mut Context<Self>) {
        self.error = Some(t!("ak.approval.notification_unavailable"));
        cx.notify();
    }
    /// Apply the client's persisted sending shortcut.
    pub fn set_send_key(&mut self, key: crate::composer::SendKey, cx: &mut Context<Self>) {
        self.composer.update(cx, |c, cx| c.set_send_key(key, cx));
    }
    /// Unlock toolbar after a confirmed or failed session configuration operation.
    pub fn selection_finished(&mut self, cx: &mut Context<Self>) {
        self.composer.update(cx, |c, cx| c.selection_finished(cx));
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
