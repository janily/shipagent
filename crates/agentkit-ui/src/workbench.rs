use crate::tr;
use agentkit_protocol::prototype::Session;
use agentkit_store::{ChatState, RunState};
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
}
/// Native fixed-layout prototype backed by actual conversation state.
pub struct WorkbenchView {
    pub(crate) app_name: SharedString,
    pub(crate) workspace: SharedString,
    pub(crate) model: Option<String>,
    pub(crate) status_key: &'static str,
    pub(crate) ready: bool,
    pub(crate) loading: bool,
    pub(crate) error: Option<String>,
    pub(crate) state: Option<ChatState>,
    pub(crate) sessions: Vec<Session>,
    pub(crate) composer: Entity<TextareaState>,
    pub(crate) scroll: Entity<MessageScrollerState>,
    pub(crate) markdown: HashMap<String, (String, Entity<TextViewState>)>,
    pub(crate) expanded: HashSet<String>,
    pub(crate) setup: bool,
    pub(crate) submitting: bool,
    configuring: bool,
    pub(crate) permission_busy: bool,
    base_url: Entity<InputState>,
    model_id: Entity<InputState>,
    api_key: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<WorkbenchIntent> for WorkbenchView {}
impl WorkbenchView {
    /// Construct the window immediately, while backend startup happens asynchronously.
    pub fn new(app_name: SharedString, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder(tr("ak.placeholder"))
                .auto_grow(2, 12)
                .submit_on_enter(true)
        });
        let input_subscription =
            cx.subscribe_in(&composer, window, |this, _, event, _, cx| match event {
                InputEvent::PressEnter { shift: false, .. } => this.submit(cx),
                InputEvent::Change => cx.notify(),
                _ => {}
            });
        Self {
            app_name,
            workspace: tr("ak.app").into(),
            model: None,
            status_key: "ak.starting",
            ready: false,
            loading: false,
            error: None,
            state: None,
            sessions: vec![],
            composer,
            scroll: cx.new(|cx| MessageScrollerState::new(0, cx)),
            markdown: HashMap::new(),
            expanded: HashSet::new(),
            setup: false,
            submitting: false,
            configuring: false,
            permission_busy: false,
            base_url: cx
                .new(|cx| InputState::new(window, cx).placeholder("http://127.0.0.1:11434/v1")),
            model_id: cx.new(|cx| InputState::new(window, cx)),
            api_key: cx.new(|cx| InputState::new(window, cx).masked(true)),
            _subscriptions: vec![input_subscription],
        }
    }
    /// Apply actual bootstrap results; no simulated connected state is used by the app.
    pub fn connected(&mut self, workspace: String, model: Option<String>, cx: &mut Context<Self>) {
        self.workspace = workspace.into();
        self.model = model;
        self.ready = true;
        self.status_key = "ak.connected";
        self.setup = self.model.is_none();
        self.error = None;
        cx.notify();
    }
    /// Update sidebar summaries from the backend.
    pub fn set_sessions(&mut self, sessions: Vec<Session>, cx: &mut Context<Self>) {
        self.sessions = sessions;
        cx.notify();
    }
    /// Replace the rendered session after pure reduction, appending only changed Markdown suffixes.
    pub fn show_session(&mut self, state: ChatState, cx: &mut Context<Self>) {
        let switched = self
            .state
            .as_ref()
            .is_none_or(|old| old.session.id != state.session.id);
        if switched {
            self.markdown.clear();
            self.expanded.clear();
        }
        if let Some(summary) = self.sessions.iter_mut().find(|s| s.id == state.session.id) {
            summary.name = state.session.name.clone();
            summary.is_processing = state.session.is_processing;
        }
        let old_count = self.scroll.read(cx).item_count();
        let count = state.session.messages.len();
        let mut first_changed = count;
        for (index, message) in state.session.messages.iter().enumerate() {
            if message.role != "assistant" {
                continue;
            }
            if let Some((previous, entity)) = self.markdown.get_mut(&message.id) {
                if previous != &message.content {
                    if message.is_streaming && message.content.starts_with(previous.as_str()) {
                        let suffix = &message.content[previous.len()..];
                        entity.update(cx, |text, cx| text.push_str(suffix, cx));
                    } else {
                        entity.update(cx, |text, cx| text.set_text(&message.content, cx));
                    }
                    *previous = message.content.clone();
                    first_changed = first_changed.min(index);
                }
            } else {
                let entity = cx.new(|cx| TextViewState::markdown(&message.content, cx));
                self.markdown
                    .insert(message.id.clone(), (message.content.clone(), entity));
                first_changed = first_changed.min(index);
            }
        }
        self.markdown.retain(|id, _| {
            state
                .session
                .messages
                .iter()
                .any(|message| &message.id == id)
        });
        self.scroll.update(cx, |scroll, cx| {
            if switched || count < old_count {
                scroll.reset(count, cx);
            } else if count > old_count {
                scroll.append(count - old_count, cx);
            }
            if first_changed < count {
                scroll.remeasure_items(first_changed..count, cx);
            }
        });
        if state.permission.is_none() {
            self.permission_busy = false;
        }
        self.error = state.last_error.clone();
        self.state = Some(state);
        self.loading = false;
        cx.notify();
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
    pub(crate) fn busy(&self) -> bool {
        self.state.as_ref().is_some_and(|state| {
            state.session.is_processing
                || matches!(
                    state.status,
                    RunState::Processing | RunState::WaitingForPermission
                )
        })
    }
    pub(crate) fn submit(&mut self, cx: &mut Context<Self>) {
        let text = self.composer.read(cx).value().to_string();
        if self.ready
            && !self.loading
            && self.model.is_some()
            && !self.busy()
            && !self.submitting
            && !text.trim().is_empty()
        {
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
            .child(div().text_lg().font_semibold().child(tr("ak.settings")))
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child(tr("ak.setup_hint")),
            )
            .child(tr("ak.url"))
            .child(
                Input::new(&self.base_url)
                    .id("endpoint-url")
                    .aria_label(tr("ak.url")),
            )
            .child(tr("ak.model"))
            .child(
                Input::new(&self.model_id)
                    .id("endpoint-model")
                    .aria_label(tr("ak.model")),
            )
            .child(tr("ak.key"))
            .child(
                Input::new(&self.api_key)
                    .id("endpoint-key")
                    .aria_label(tr("ak.key")),
            )
            .child(
                Button::new("save-endpoint")
                    .primary()
                    .label(tr("ak.save"))
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
                        .label(tr("ak.back"))
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.busy();
        let title = self
            .state
            .as_ref()
            .and_then(|s| s.session.name.clone())
            .unwrap_or_else(|| self.app_name.to_string());
        let sidebar = v_flex()
            .w(px(280.))
            .flex_shrink_0()
            .h_full()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().border)
            .p_3()
            .gap_3()
            .child(
                div()
                    .h_9()
                    .flex()
                    .items_center()
                    .px_3()
                    .rounded_md()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(self.workspace.clone()),
            )
            .child(
                Button::new("new-session")
                    .primary()
                    .w_full()
                    .label(tr("ak.new"))
                    .disabled(!self.ready || busy || self.submitting || self.loading)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(WorkbenchIntent::NewSession))),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(tr("ak.sessions")),
            )
            .child(
                v_flex()
                    .id("session-list")
                    .flex_1()
                    .overflow_y_scroll()
                    .gap_1()
                    .children(self.sessions.iter().map(|session| {
                        let id = session.id.clone();
                        let active = self.state.as_ref().is_some_and(|s| s.session.id == id);
                        Button::new(SharedString::from(format!("session:{id}")))
                            .ghost()
                            .w_full()
                            .justify_start()
                            .label(
                                session
                                    .name
                                    .clone()
                                    .or(session.preview.clone())
                                    .unwrap_or_else(|| tr("ak.new")),
                            )
                            .disabled(busy || self.submitting || self.loading)
                            .when(active, |button| button.bg(cx.theme().sidebar_accent))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(WorkbenchIntent::OpenSession(id.clone()))
                            }))
                    })),
            )
            .child(
                Button::new("model-settings")
                    .ghost()
                    .label(tr("ak.settings"))
                    .disabled(busy)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.setup = true;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("toggle-theme")
                    .ghost()
                    .label(tr("ak.theme"))
                    .on_click(|_, window, cx| {
                        let mode = if cx.theme().mode.is_dark() {
                            ThemeMode::Light
                        } else {
                            ThemeMode::Dark
                        };
                        Theme::change(mode, Some(window), cx);
                    }),
            );
        let content = if self.setup {
            h_flex()
                .id("endpoint-scroll")
                .flex_1()
                .min_h_0()
                .justify_center()
                .overflow_y_scroll()
                .child(self.endpoint_form(cx))
                .into_any_element()
        } else {
            self.chat_body(cx)
        };
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_size(px(14.))
            .child(
                TitleBar::new().child(
                    h_flex()
                        .flex_1()
                        .justify_center()
                        .child(self.app_name.clone()),
                ),
            )
            .child(
                h_flex().flex_1().min_h_0().child(sidebar).child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .child(
                            h_flex()
                                .h(px(48.))
                                .px_6()
                                .border_b_1()
                                .border_color(cx.theme().border)
                                .justify_between()
                                .child(div().font_semibold().child(title))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(
                                            self.state
                                                .as_ref()
                                                .and_then(|s| s.session.working_directory.clone())
                                                .unwrap_or_default(),
                                        ),
                                ),
                        )
                        .when_some(self.error.clone(), |view, error| {
                            view.child(
                                h_flex()
                                    .px_6()
                                    .py_2()
                                    .gap_3()
                                    .bg(cx.theme().muted)
                                    .child(
                                        div().flex_1().text_color(cx.theme().danger).child(error),
                                    )
                                    .when(!self.ready, |view| {
                                        view.child(
                                            Button::new("retry")
                                                .outline()
                                                .label(tr("ak.retry"))
                                                .on_click(cx.listener(|_, _, _, cx| {
                                                    cx.emit(WorkbenchIntent::Reconnect)
                                                })),
                                        )
                                    }),
                            )
                        })
                        .child(content),
                ),
            )
            .child(
                h_flex()
                    .h(px(24.))
                    .px_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().sidebar)
                    .justify_between()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        div()
                            .id("connection-status")
                            .role(Role::Status)
                            .aria_label(tr(self.status_key))
                            .child(tr(self.status_key))
                            .test_support(),
                    )
                    .child(self.model.clone().unwrap_or_else(|| tr("ak.no_model"))),
            )
    }
}
