use crate::t;
#[path = "shell.rs"]
mod shell;
use crate::layout::ShellPreferences;
use crate::transcript::{ChatIntent, ChatView};
use agentkit_store::entities::{AppState, SessionListStore, SessionStore, WorkspaceStore};
use gpui_kit::component::resizable::ResizableState;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    message_scroller::MessageScrollerState,
    *,
};
use gpui_kit::{prelude::*, *};

/// Semantic UI actions, handled only by the app controller.
#[derive(Clone)]
pub enum WorkbenchIntent {
    /// Restart the owned backend, confirming interruption when needed.
    RestartBackend,
    /// Retry loading the same requested conversation.
    RetrySession,
    /// Open only the client-owned backend log directory.
    OpenRecoveryLogs,
    /// Export a redacted, whitelist-only recovery report.
    ExportRecovery,
    /// Ask the app to confirm and perform an orderly window close.
    Quit,
    /// Native first-run setup gesture.
    Onboarding(crate::onboarding::OnboardingIntent),
    /// Native settings operation.
    Settings(crate::settings::SettingsIntent),
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
    /// Retry backend startup or connection after failure.
    Reconnect,
    /// Persist a user shell layout change locally.
    LayoutChanged(ShellPreferences),
    /// Cycle system/light/dark appearance in the application layer.
    CycleAppearance,
}
/// Native application shell hosting the canonical conversation components.
pub struct WorkbenchView {
    pub(crate) connection: crate::recovery::Phase,
    pub(crate) can_restart: bool,
    pub(crate) loading_visible: bool,
    load_revision: u64,
    pub(crate) session_error: Option<String>,
    pub(crate) center_focus: FocusHandle,
    pub(crate) menu_bar: Option<Entity<gpui_kit::component::menu::AppMenuBar>>,
    pub(crate) font_size: u8,
    pub(crate) onboarding_view: Option<Entity<crate::onboarding::OnboardingView>>,
    onboarding_subscription: Option<Subscription>,
    pub(crate) settings_view: Entity<crate::settings::SettingsView>,
    pub(crate) settings_open: bool,
    settings_previous_focus: Option<FocusHandle>,
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
    pub(crate) loading: bool,
    pub(crate) error: Option<String>,
    pub(crate) state: Option<Entity<SessionStore>>,
    pub(crate) sessions: Option<Entity<SessionListStore>>,
    pub(crate) composer: Entity<crate::composer::Composer>,
    pub(crate) scroll: Entity<MessageScrollerState>,
    pub(crate) chat: Entity<ChatView>,
    pub(crate) setup: bool,
    pub(crate) submitting: bool,
    pub(crate) permission_prompt: Entity<crate::permissions::PermissionPrompt>,
    _subscriptions: Vec<Subscription>,
    session_subscriptions: Vec<Subscription>,
    list_subscription: Option<Subscription>,
    pub(crate) app_state: Option<Entity<AppState>>,
    pub(crate) workspace_state: Option<Entity<WorkspaceStore>>,
    context_subscriptions: Vec<Subscription>,
}
impl EventEmitter<WorkbenchIntent> for WorkbenchView {}
impl WorkbenchView {
    /// Synchronize the composer's legacy Escape handling with effective app shortcuts.
    pub fn set_escape_stops(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.composer.update(cx, |c, _| c.set_escape_stops(enabled));
    }
    /// Share the native menu model with the Windows/Linux title bar.
    pub fn set_menu_bar(
        &mut self,
        bar: Option<Entity<gpui_kit::component::menu::AppMenuBar>>,
        cx: &mut Context<Self>,
    ) {
        self.menu_bar = bar;
        cx.notify();
    }
    /// Route a menu mode gesture through the composer's existing transition policy.
    pub fn cycle_mode_from_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.conversation_visible() {
            self.composer
                .update(cx, |c, cx| c.cycle_from_menu(window, cx));
        }
    }
    /// Whether conversation commands can target a visible conversation.
    pub fn conversation_visible(&self) -> bool {
        self.onboarding_view.is_none()
            && !self.settings_open
            && !matches!(self.connection, crate::recovery::Phase::Fatal { .. })
    }
    /// Whether first-run setup owns the window.
    pub fn onboarding_visible(&self) -> bool {
        self.onboarding_view.is_some()
    }
    /// Construct the window immediately, while backend startup happens asynchronously.
    pub fn new(app_name: SharedString, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings_view = cx.new(crate::settings::SettingsView::new);
        let settings_subscription = cx.subscribe(&settings_view, |_, _, intent, cx| {
            cx.emit(WorkbenchIntent::Settings(intent.clone()))
        });
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
            connection: crate::recovery::Phase::Starting,
            can_restart: false,
            loading_visible: false,
            load_revision: 0,
            session_error: None,
            center_focus: cx.focus_handle(),
            menu_bar: None,
            onboarding_view: None,
            onboarding_subscription: None,
            settings_view,
            font_size: 14,
            settings_open: false,
            settings_previous_focus: None,
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
            loading: false,
            error: None,
            state: None,
            sessions: None,
            composer,
            scroll,
            chat,
            setup: false,
            submitting: false,
            permission_prompt,
            _subscriptions: vec![
                settings_subscription,
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
        if self.onboarding_view.is_some() {
            return;
        }
        self.save_draft(cx);
        self.permission_prompt.update(cx, |p, cx| p.suspend(cx));
        self.sidebar_sheet = false;
        if self.sheet_kind.take().is_some() {
            window.close_sheet(cx);
        }
        if !self.settings_open {
            self.settings_previous_focus = window.focused(cx);
        }
        self.settings_open = true;
        let settings = self.settings_view.clone();
        window.defer(cx, move |window, cx| {
            settings.update(cx, |s, cx| s.focus(window, cx))
        });
        cx.emit(WorkbenchIntent::Settings(
            crate::settings::SettingsIntent::Reload,
        ));
        cx.notify();
    }
    /// Return to the retained canonical conversation and its draft.
    pub fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_open = false;
        if let Some(focus) = self.settings_previous_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }
    /// Apply instance-owned display settings to retained conversation views.
    pub fn set_display(&mut self, font: u8, fade: bool, cx: &mut Context<Self>) {
        self.font_size = font;
        self.chat.update(cx, |c, cx| c.set_stream_fade(fade, cx));
        let count = self.scroll.read(cx).item_count();
        self.scroll
            .update(cx, |s, cx| s.remeasure_items(0..count, cx));
        cx.notify();
    }
    /// Refresh controls that retain translated input placeholders.
    pub fn localize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_view.update(cx, |s, cx| s.localize(window, cx));
        cx.notify();
    }
    /// Controller access to the native settings Entity.
    pub fn settings(&self) -> Entity<crate::settings::SettingsView> {
        self.settings_view.clone()
    }
    /// Whether the settings replacement page is visible.
    pub fn settings_visible(&self) -> bool {
        self.settings_open
    }
    /// Apply actual bootstrap results; no simulated connected state is used by the app.
    pub fn connected(&mut self, workspace: String, model: Option<String>, cx: &mut Context<Self>) {
        self.workspace = workspace.into();
        self.model = model;
        self.ready = true;
        self.connection = crate::recovery::Phase::Online;
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
    /// Observe global and workspace context without retaining network handles.
    pub fn bind_context(
        &mut self,
        app: Entity<AppState>,
        workspace: Entity<WorkspaceStore>,
        cx: &mut Context<Self>,
    ) {
        self.context_subscriptions.clear();
        self.app_state = Some(app.clone());
        self.settings_view
            .update(cx, |s, cx| s.bind(app.clone(), cx));
        self.workspace_state = Some(workspace.clone());
        self.sidebar_view
            .update(cx, |s, cx| s.bind_workspace(workspace.clone(), cx));
        self.context_subscriptions
            .push(cx.observe(&app, |this, app, cx| {
                let catalog = &app.read(cx).connections;
                let (model, _) = crate::composer::resolved_model(catalog, None, None);
                this.model = model;
                this.setup = this.model.is_none();
                cx.notify();
            }));
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
        self.loading_visible = false;
        self.session_error = None;
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
        if !self.setup
            && !self.settings_visible()
            && !self.sidebar_view.read(cx).focused(window, cx)
        {
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
    #[cfg(any(feature = "samples", feature = "e2e"))]
    pub fn composer_input(&self, cx: &App) -> Entity<gpui_kit::component::input::TextareaState> {
        self.composer.read(cx).input().clone()
    }
    /// Inspect virtual-list state in component and performance sample builds.
    #[cfg(any(feature = "samples", feature = "e2e"))]
    pub fn scroll_state(&self) -> &Entity<MessageScrollerState> {
        &self.scroll
    }
    /// Inspect rendered Markdown content in component/performance sample builds.
    #[cfg(any(feature = "samples", feature = "e2e"))]
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
    /// Present a lifecycle or operation failure.
    pub fn show_error(&mut self, error: String, disconnected: bool, cx: &mut Context<Self>) {
        self.error = Some(error);
        self.submitting = false;
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
        self.loading_visible = false;
        self.session_error = None;
        self.load_revision = self.load_revision.wrapping_add(1);
        let revision = self.load_revision;
        cx.spawn(async move |weak, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(150))
                .await;
            let _ = weak.update(cx, |v, cx| {
                if v.loading && v.load_revision == revision {
                    v.loading_visible = true;
                    cx.notify();
                }
            });
        })
        .detach();
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
}
impl Render for WorkbenchView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_shell(window, cx)
    }
}

impl WorkbenchView {
    /// Show a full-window wizard and forward only semantic gestures.
    pub fn show_onboarding(&mut self, cx: &mut Context<Self>) {
        let view = cx.new(crate::onboarding::OnboardingView::new);
        self.onboarding_subscription = Some(cx.subscribe(&view, |_, _, intent, cx| {
            cx.emit(WorkbenchIntent::Onboarding(intent.clone()))
        }));
        self.onboarding_view = Some(view);
        cx.notify();
    }
    /// Access the active wizard, if any.
    pub fn onboarding(&self) -> Option<Entity<crate::onboarding::OnboardingView>> {
        self.onboarding_view.clone()
    }
    /// Drop wizard state and focus the first conversation input.
    pub fn close_onboarding(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.onboarding_view = None;
        self.onboarding_subscription = None;
        self.composer
            .read(cx)
            .input()
            .clone()
            .update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }
}

#[cfg(feature = "e2e")]
impl WorkbenchView {
    /// Report the same cached readiness used by the displayed Composer controls.
    pub fn e2e_composer_ready(&self, cx: &App) -> bool {
        self.composer.read(cx).e2e_ready()
    }
    /// Drive the normal Composer submit path, including its IME/model/session gates.
    pub fn e2e_send(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.composer
            .read(cx)
            .input()
            .clone()
            .update(cx, |input, cx| input.set_value(text, window, cx));
        self.composer.update(cx, |c, cx| c.submit(window, cx));
    }
}
