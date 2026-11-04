#[path = "approval_notifications.rs"]
mod approval_notifications;
#[path = "chat_actions.rs"]
mod chat_actions;
#[path = "composer_actions.rs"]
mod composer_actions;
#[path = "list_refresh.rs"]
mod list_refresh;
#[path = "navigation.rs"]
mod navigation;
#[cfg(feature = "perf")]
#[path = "perf.rs"]
mod perf;
#[path = "permission_actions.rs"]
mod permission_actions;
#[path = "tool_content.rs"]
mod tool_content;
#[cfg(feature = "perf")]
pub(crate) fn validate_perf() -> Result<()> {
    perf::Config::from_env().map(|_| ())
}
use crate::{
    Brand,
    actions::*,
    preferences::{Appearance, Geometry, PreferenceStore, Preferences},
};
use agentkit_client::recording::RecordingInput;
use agentkit_client::{ConnectionState, Endpoint, api::WorkbenchApi};
use agentkit_protocol::{DecodedSessionEvent, generated::Session, generated::Workspace};
use agentkit_sidecar::{SidecarConfig, SidecarEvent, SidecarHandle};
use agentkit_store::{
    core::{
        reducer::{Context as ReductionContext, Effect},
        session::MAX_BATCH,
    },
    entities::{AppState, SessionListStore, SessionStore, WorkspaceStore},
};
type RecordingSender = async_channel::Sender<RecordingInput>;
use agentkit_ui::{WorkbenchIntent, WorkbenchView};
use anyhow::{Context as _, Result};
use gpui_kit::component::WindowExt;
use gpui_kit::*;
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    time::Duration,
};

fn report_recording_failure() {
    // Development output only; never include transcript, token or destination path.
    eprintln!("Event recording stopped: destination exists or IO failed");
}
fn sidecar_message(error: &agentkit_sidecar::SidecarError) -> String {
    use agentkit_sidecar::SidecarError as E;
    match error {
        E::MissingRuntime(_) => agentkit_ui::t!("ak.sidecar_missing_runtime"),
        E::Io(_) => agentkit_ui::t!("ak.sidecar_io"),
        E::ReadyTimeout => agentkit_ui::t!("ak.sidecar_ready_timeout"),
        E::ExitedEarly { .. } => agentkit_ui::t!("ak.sidecar_exited_early"),
        E::AlreadyRunning { .. } => agentkit_ui::t!("ak.sidecar_occupied"),
        E::InvalidEndpoint => agentkit_ui::t!("ak.sidecar_invalid_endpoint"),
        E::RestartLimit => agentkit_ui::t!("ak.sidecar_restart_limit"),
        E::Stopped => agentkit_ui::t!("ak.sidecar_stopped"),
    }
}
pub(crate) type StartupSlot = Rc<RefCell<Option<Task<()>>>>;
pub(crate) type RuntimeSlot = Rc<RefCell<Option<Running>>>;
pub(crate) struct Running {
    api: WorkbenchApi,
    sidecar: Option<SidecarHandle>,
    lifecycle: Option<smol::Task<()>>,
    recording: Option<smol::Task<()>>,
    session_id: Option<String>,
}
impl Running {
    pub(crate) async fn shutdown_for_exit(mut self) {
        self.recording.take();
        self.lifecycle.take();
        // GPUI allows only 200 ms in on_app_quit. Close transport first and
        // reserve time to kill and reap the owned process group.
        self.api.close();
        if let Some(sidecar) = self.sidecar {
            let _ = sidecar.shutdown(Duration::from_millis(60)).await;
        }
    }
    pub(crate) async fn shutdown(mut self) {
        self.recording.take();
        self.lifecycle.take();
        if let Some(id) = &self.session_id {
            let _ = smol::future::race(self.api.cancel_for_shutdown(id), async {
                smol::Timer::after(Duration::from_millis(500)).await;
                Ok(())
            })
            .await;
        }
        self.api.close();
        if let Some(sidecar) = self.sidecar {
            let _ = sidecar.shutdown(Duration::from_secs(5)).await;
        }
    }
}
struct SharedAppState(Entity<AppState>);
impl Global for SharedAppState {}
struct ContextEvents {
    unread: async_channel::Receiver<
        Result<agentkit_protocol::generated::UnreadSummary, agentkit_client::RpcError>,
    >,
    skills: async_channel::Receiver<
        Result<(String, Vec<agentkit_protocol::generated::LoadedSkill>), agentkit_client::RpcError>,
    >,
    connections: async_channel::Receiver<Result<(), agentkit_client::RpcError>>,
}
struct Started {
    workspaces: Vec<Workspace>,
    running: Running,
    context: ContextEvents,
    workspace: Workspace,
    sessions: Vec<Session>,
    session: Session,
    model: Option<String>,
    events: async_channel::Receiver<Vec<serde_json::Value>>,
    settings: Option<agentkit_protocol::generated::WorkspaceSettings>,
    unread: f64,
    skills: Vec<agentkit_protocol::generated::LoadedSkill>,
    connections: Vec<agentkit_protocol::generated::LlmConnectionWithStatus>,
}
async fn bootstrap(
    brand: Brand,
    recording: Option<RecordingSender>,
    preferences: Preferences,
) -> Result<Started> {
    let mut sidecar = None;
    let endpoint = match (
        std::env::var("AGENTKIT_REMOTE_URL"),
        std::env::var("AGENTKIT_REMOTE_TOKEN"),
    ) {
        (Ok(url), Ok(token)) => Endpoint {
            url,
            token: token.into(),
        },
        (Err(_), Err(_)) => {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
            let platform = match std::env::consts::OS {
                "macos" => "darwin",
                "windows" => "win32",
                other => other,
            };
            let arch = if cfg!(target_arch = "aarch64") {
                "arm64"
            } else {
                "x64"
            };
            let backend = std::env::var_os("AGENTKIT_BACKEND_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| root.join(format!("target/backend/{platform}-{arch}")));
            let config = if let Some(path) = std::env::var_os("AGENTKIT_CONFIG_DIR") {
                PathBuf::from(path)
            } else {
                let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                    .context("User profile directory unavailable")?;
                PathBuf::from(home).join(&brand.data_dir_name)
            };
            let config = SidecarConfig::new(backend, config);
            #[cfg(target_os = "macos")]
            let config = {
                let mut config = config;
                if let Some(home) = std::env::var_os("HOME") {
                    config.log_dir = PathBuf::from(home)
                        .join("Library/Logs")
                        .join(&brand.bundle_id);
                }
                config
            };
            let handle = SidecarHandle::start(config).await?;
            let endpoint = handle.endpoint();
            sidecar = Some(handle);
            endpoint
        }
        _ => anyhow::bail!("Both AGENTKIT_REMOTE_URL and AGENTKIT_REMOTE_TOKEN are required"),
    };
    match initialize(endpoint, recording.clone(), &preferences).await {
        Ok(mut started) => {
            if let Some(handle) = &sidecar {
                let events = handle.events();
                let api = started.running.api.clone();
                started.running.lifecycle = Some(smol::spawn(async move {
                    while let Ok(event) = events.recv().await {
                        match event {
                            SidecarEvent::Restarted { endpoint } => {
                                if let Some(tx) = &recording {
                                    let _ = tx
                                        .send(RecordingInput::Redact(endpoint.token.clone()))
                                        .await;
                                }
                                api.replace_endpoint(endpoint);
                            }
                            SidecarEvent::Fatal { .. } => api.close(),
                            SidecarEvent::Exited { .. } => {}
                        }
                    }
                }));
            }
            started.running.sidecar = sidecar;
            Ok(started)
        }
        Err(error) => {
            // Do not let a failed bootstrap finish before its child is reaped.
            if let Some(sidecar) = sidecar {
                let _ = sidecar.shutdown(Duration::from_secs(5)).await;
            }
            Err(error)
        }
    }
}
async fn initialize(
    endpoint: Endpoint,
    recording: Option<RecordingSender>,
    preferences: &Preferences,
) -> Result<Started> {
    let initial = WorkbenchApi::connect(endpoint.clone(), String::new());
    initial.health().await?;
    let workspaces = initial.workspaces_generated().await?;
    let selected = if preferences.restore_session {
        workspaces
            .iter()
            .position(|w| Some(&w.id) == preferences.workspace.as_ref())
            .unwrap_or(0)
    } else {
        0
    };
    anyhow::ensure!(!workspaces.is_empty(), agentkit_ui::t!("ak.no_workspace"));
    let workspace = workspaces[selected].clone();
    initial.disable_browser().await?;
    let connections = initial.connections_generated().await?;
    let model = connections
        .iter()
        .find(|c| c.is_authenticated && c.is_default == Some(true))
        .or_else(|| connections.iter().find(|c| c.is_authenticated))
        .and_then(|c| {
            c.default_model.clone().or_else(|| {
                c.models.as_ref()?.first().map(|m| match m {
                    agentkit_protocol::generated::LlmConnectionWithStatusModelsItem::String(id) => {
                        id.clone()
                    }
                    agentkit_protocol::generated::LlmConnectionWithStatusModelsItem::Object {
                        id,
                        ..
                    } => id.clone(),
                })
            })
        });
    initial.close();
    if let Some(tx) = &recording {
        let _ = tx
            .send(RecordingInput::Redact(endpoint.token.clone()))
            .await;
    }
    let api = WorkbenchApi::connect(endpoint, workspace.id.clone());
    let recording = recording.map(|tx| {
        let receiver = api.session_events();
        smol::spawn(async move {
            while let Ok(args) = receiver.recv().await {
                if tx.send(RecordingInput::Events(args)).await.is_err() {
                    report_recording_failure();
                    break;
                }
            }
        })
    });
    let events = api.session_events();
    let context = ContextEvents {
        unread: api.unread_events(),
        skills: api.skill_events(),
        connections: api.connection_events(),
    };
    let unread = api.unread_snapshot().await?.total_unread_sessions;
    let settings = api.workspace_settings(&workspace.id).await?;
    let skills = api.workspace_skills(&workspace.id).await?;
    let mut sessions = api.sessions_generated().await?;
    let session = if let Some(summary) = restored_session(&sessions, preferences) {
        api.session_generated(&summary.id).await?
    } else {
        let working_directory = std::env::var("AGENTKIT_WORKING_DIR").ok();
        let session = api
            .create_session_generated(&workspace.id, working_directory.as_deref())
            .await?;
        sessions.push(session.clone());
        session
    };
    let running = Running {
        api,
        sidecar: None,
        lifecycle: None,
        recording,
        session_id: Some(session.id.clone()),
    };
    Ok(Started {
        running,
        workspaces,
        context,
        workspace,
        sessions,
        session,
        model,
        events,
        settings,
        unread,
        skills,
        connections,
    })
}

pub(crate) struct WorkbenchController {
    #[cfg(feature = "perf")]
    perf: Option<perf::Capture>,
    view: Entity<WorkbenchView>,
    brand: Brand,
    runtime: RuntimeSlot,
    workspace: Option<Workspace>,
    workspace_sessions: std::collections::HashMap<String, String>,
    state: Option<Entity<SessionStore>>,
    stores: std::collections::HashMap<String, Entity<SessionStore>>,
    app_state: Entity<AppState>,
    workspace_store: Entity<WorkspaceStore>,
    list: Entity<SessionListStore>,
    backlog: Vec<(DecodedSessionEvent, ReductionContext)>,
    backlog_overflow: bool,
    list_refresh: list_refresh::ListRefresh,
    list_backlog: Vec<DecodedSessionEvent>,
    list_overflow: bool,
    epoch: u64,
    context_events: Vec<Task<()>>,
    drafts: std::collections::HashMap<
        String,
        (
            agentkit_protocol::generated::SessionDraft,
            Vec<agentkit_protocol::generated::StoredAttachment>,
        ),
    >,
    quitting: Rc<Cell<bool>>,
    loading: bool,
    workspace_switching: bool,
    transport_epoch: u64,
    startup: StartupSlot,
    events: Option<Task<()>>,
    monitor: Option<Task<()>>,
    viewing: Option<Task<()>>,
    operations: Vec<Task<()>>,
    approval_notifications: approval_notifications::ApprovalNotifications,
    recording_sender: Option<RecordingSender>,
    _recording_worker: Option<smol::Task<()>>,
    preferences: PreferenceStore,
    focus_handle: FocusHandle,
    _window_observers: Vec<Subscription>,
    _preference_monitor: Option<Task<()>>,
    _subscription: Subscription,
    chat_dialog_subscription: Option<Subscription>,
    navigation_subscription: Option<Subscription>,
    navigation_pending: bool,
}
impl WorkbenchController {
    pub(crate) fn new(
        brand: Brand,
        runtime: RuntimeSlot,
        startup: StartupSlot,
        quitting: Rc<Cell<bool>>,
        preferences: PreferenceStore,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let view = cx.new(|cx| WorkbenchView::new(brand.app_name.clone().into(), window, cx));
        view.update(cx, |v, cx| {
            v.set_send_key(preferences.value.borrow().send_key, cx)
        });
        let subscription = cx.subscribe_in(&view, window, |this, _, intent, window, cx| {
            this.handle(intent.clone(), window, cx)
        });
        let app_state = if let Some(shared) = cx.try_global::<SharedAppState>() {
            shared.0.clone()
        } else {
            let entity = cx.new(|_| AppState::default());
            cx.set_global(SharedAppState(entity.clone()));
            entity
        };
        let (recording_sender, recording_worker) =
            if let Some(path) = std::env::var_os("AGENTKIT_RECORD_EVENTS") {
                let (tx, rx) = async_channel::bounded(256);
                let worker = smol::spawn(async move {
                    if agentkit_client::recording::record(rx, path.into())
                        .await
                        .is_err()
                    {
                        report_recording_failure();
                    }
                });
                (Some(tx), Some(worker))
            } else {
                (None, None)
            };
        let mut this = Self {
            #[cfg(feature = "perf")]
            perf: perf::Config::from_env()
                .ok()
                .flatten()
                .map(|config| perf::Capture::new(config, window.window_handle().window_id())),
            view,
            brand,
            runtime,
            workspace: None,
            workspace_sessions: Default::default(),
            state: None,
            stores: std::collections::HashMap::new(),
            app_state,
            workspace_store: cx.new(|_| WorkspaceStore::default()),
            list: cx.new(|_| SessionListStore::default()),
            backlog: Vec::new(),
            backlog_overflow: false,
            list_refresh: Default::default(),
            list_backlog: Vec::new(),
            list_overflow: false,
            epoch: 0,
            context_events: Vec::new(),
            drafts: std::collections::HashMap::new(),
            quitting,
            loading: false,
            workspace_switching: false,
            transport_epoch: 0,
            startup,
            events: None,
            monitor: None,
            viewing: None,
            operations: vec![],
            approval_notifications: Default::default(),
            recording_sender,
            _recording_worker: recording_worker,
            _subscription: subscription,
            chat_dialog_subscription: None,
            navigation_subscription: None,
            navigation_pending: false,
            preferences,
            focus_handle: cx.focus_handle(),
            _window_observers: Vec::new(),
            _preference_monitor: None,
        };
        this.focus_handle.focus(window, cx);
        this.view.update(cx, |view, cx| {
            view.apply_shell(
                this.preferences.value.borrow().shell.clone(),
                this.preferences.writable(),
                window,
                cx,
            )
        });
        this._window_observers
            .push(cx.observe_window_bounds(window, |this, window, cx| {
                let bounds = window.window_bounds();
                let b = bounds.get_bounds();
                this.preferences.update(|p| {
                    p.geometry = Some(Geometry {
                        x: f32::from(b.origin.x),
                        y: f32::from(b.origin.y),
                        width: f32::from(b.size.width),
                        height: f32::from(b.size.height),
                        maximized: matches!(bounds, WindowBounds::Maximized(_)),
                    })
                });
                this.view.update(cx, |v, cx| v.viewport_changed(window, cx));
            }));
        this._window_observers
            .push(cx.observe_window_appearance(window, |this, window, cx| {
                if this.preferences.value.borrow().appearance == Appearance::System {
                    gpui_kit::component::Theme::sync_system_appearance(Some(window), cx);
                }
            }));
        let failures = this.preferences.failures();
        this._preference_monitor = Some(cx.spawn(async move |weak, cx| {
            while failures.recv().await.is_ok() {
                let _ = weak.update(cx, |this, cx| {
                    this.view
                        .update(cx, |v, cx| v.set_preferences_writable(false, cx))
                });
            }
        }));
        this.start(window, cx);
        #[cfg(feature = "perf")]
        perf::watchdog(&mut this, window, cx);
        this
    }
    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.quitting.get() {
            return;
        }
        if self
            .startup
            .borrow()
            .as_ref()
            .is_some_and(|task| !task.is_ready())
        {
            return;
        }
        self.save_draft(cx);
        self.epoch = self.epoch.wrapping_add(1);
        self.loading = false;
        self.navigation_pending = false;
        self.workspace_switching = false;
        self.list_refresh.invalidate();
        self.list_backlog.clear();
        self.list_overflow = false;
        self.context_events.clear();
        self.events = None;
        self.monitor = None;
        self.viewing = None;
        let previous = self.runtime.borrow_mut().take();
        let brand = self.brand.clone();
        let recording = self.recording_sender.clone();
        let preferences = bootstrap_preferences(
            &self.preferences.value.borrow(),
            self.state.as_ref().map(|s| &s.read(cx).model.state.session),
        );
        let quitting = self.quitting.clone();
        *self.startup.borrow_mut() = Some(cx.spawn_in(window, async move |weak, cx| {
            if let Some(previous) = previous {
                previous.shutdown().await;
            }
            // Environment and filesystem work stays off the GPUI foreground thread.
            let result = cx
                .background_executor()
                .spawn(async move { bootstrap(brand, recording, preferences).await })
                .await;
            if quitting.get() {
                if let Ok(started) = result {
                    started.running.shutdown().await;
                }
                return;
            }
            let mut result = Some(result);
            let _ = weak.update_in(cx, |this, window, cx| match result.take() {
                Some(Ok(started)) => this.started(started, window, cx),
                Some(Err(error)) => this.view.update(cx, |view, cx| {
                    view.show_error(
                        error
                            .downcast_ref::<agentkit_sidecar::SidecarError>()
                            .map(sidecar_message)
                            .unwrap_or_else(|| error.to_string()),
                        true,
                        cx,
                    )
                }),
                None => {}
            });
            if let Some(Ok(started)) = result {
                started.running.shutdown().await;
            }
        }));
    }
    fn started(&mut self, started: Started, window: &mut Window, cx: &mut Context<Self>) {
        for (id, store) in &self.stores {
            self.drafts.insert(
                id.clone(),
                (
                    store.read(cx).model.draft.clone(),
                    store.read(cx).model.draft_attachments.clone(),
                ),
            );
        }
        self.stores.clear();
        self.backlog.clear();
        let mut initial = SessionStore::new(started.session);
        if let Some((draft, attachments)) = self.drafts.remove(&initial.model.state.session.id) {
            initial.model.draft = draft;
            initial.model.draft_attachments = attachments;
        }
        let state = cx.new(|_| initial);
        self.stores
            .insert(state.read(cx).model.state.session.id.clone(), state.clone());
        self.state = Some(state.clone());
        self.workspace = Some(started.workspace.clone());
        self.workspace_store.update(cx, |s, cx| {
            s.catalog = started.workspaces;
            s.id = started.workspace.id.clone();
            s.name = started.workspace.name.clone();
            s.workspace = Some(started.workspace.clone());
            s.settings = started.settings;
            s.skills = started.skills;
            cx.notify();
        });
        self.list
            .update(cx, |s, cx| s.replace(started.sessions, cx));
        let api = started.running.api.clone();
        self.transport_epoch = api.snapshot().recovery_id;
        let info = api.snapshot();
        self.app_state.update(cx, |s, cx| {
            s.connected = true;
            s.background_tasks = 0;
            s.connections = started.connections;
            s.unread = started.unread;
            s.theme = format!("{:?}", self.preferences.value.borrow().appearance).to_lowercase();
            s.language = self.preferences.value.borrow().language.clone();
            s.server_version = info.server_version;
            s.registered_channels = info.registered_channels.unwrap_or_default();
            cx.notify();
        });
        self.preferences.update(|p| {
            p.workspace = Some(started.workspace.id.clone());
            p.session = Some(state.read(cx).model.state.session.id.clone());
        });
        *self.runtime.borrow_mut() = Some(started.running);
        self.view.update(cx, |view, cx| {
            view.connected(started.workspace.name, started.model, cx);
            view.bind_list(self.list.clone(), cx);
            view.bind_context(self.app_state.clone(), self.workspace_store.clone(), cx);
            view.bind_session(state, window, cx);
        });
        #[cfg(feature = "perf")]
        perf::launch(self, window, cx);
        let events = started.events;
        self.events = Some(cx.spawn_in(window, async move |weak, cx| {
            while let Ok(first) = events.recv().await {
                #[cfg(feature = "perf")]
                let received = std::time::Instant::now();
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let mut batch = vec![first];
                while batch.len() < MAX_BATCH {
                    match events.try_recv() {
                        Ok(e) => batch.push(e),
                        Err(_) => break,
                    }
                }
                if weak
                    .update_in(cx, |this, window, cx| {
                        #[cfg(feature = "perf")]
                        let update_started = std::time::Instant::now();
                        let mut decoded = Vec::new();
                        for args in batch {
                            if let Some(value) = args.into_iter().next() {
                                #[cfg(feature = "perf")]
                                if let Ok(event) = serde_json::from_value::<
                                    agentkit_protocol::prototype::SessionEvent,
                                >(value.clone())
                                    && let Some(capture) = &mut this.perf
                                {
                                    capture.observe(&event);
                                }
                                match serde_json::from_value::<DecodedSessionEvent>(value) {
                                    Ok(e) => decoded.push((e, reduction_context())),
                                    Err(_) => this.view.update(cx, |v, cx| {
                                        v.show_error(agentkit_ui::t!("ak.event_invalid"), false, cx)
                                    }),
                                }
                            }
                        }
                        this.route(decoded, window, cx);
                        #[cfg(feature = "perf")]
                        if let Some(capture) = &mut this.perf {
                            capture.note_update(
                                received,
                                update_started,
                                window.is_window_active(),
                            );
                        }
                        #[cfg(feature = "perf")]
                        perf::finish_if_terminal(this, window, cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        let ContextEvents {
            unread,
            skills,
            connections,
        } = started.context;
        let catalog = api.clone();
        self.context_events.push(cx.spawn(async move |weak, cx| {
            while let Ok(update) = unread.recv().await {
                if weak
                    .update(cx, |this, cx| match update {
                        Ok(update) => this.app_state.update(cx, |s, cx| {
                            s.unread = update.total_unread_sessions;
                            cx.notify();
                        }),
                        Err(_) => this.view.update(cx, |v, cx| {
                            v.show_error(agentkit_ui::t!("ak.event_invalid"), false, cx)
                        }),
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        self.context_events.push(cx.spawn(async move |weak, cx| {
            while let Ok(update) = skills.recv().await {
                if weak
                    .update(cx, |this, cx| match update {
                        Ok((id, skills)) => this.workspace_store.update(cx, |s, cx| {
                            if s.id == id {
                                s.skills = skills;
                                cx.notify();
                            }
                        }),
                        Err(_) => this.view.update(cx, |v, cx| {
                            v.show_error(agentkit_ui::t!("ak.event_invalid"), false, cx)
                        }),
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        self.context_events.push(cx.spawn(async move |weak, cx| {
            while let Ok(update) = connections.recv().await {
                let result = match update {
                    Ok(()) => catalog.connections_generated().await,
                    Err(error) => Err(error),
                };
                if weak
                    .update(cx, |this, cx| match result {
                        Ok(connections) => this.app_state.update(cx, |s, cx| {
                            s.connections = connections;
                            cx.notify();
                        }),
                        Err(_) => this.view.update(cx, |v, cx| {
                            v.show_error(agentkit_ui::t!("ak.event_invalid"), false, cx)
                        }),
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        self.mark_viewed(window, cx);
        let mut observed = api.snapshot();
        self.monitor = Some(cx.spawn_in(window, async move |weak, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let snapshot = api.snapshot();
                if snapshot.state != observed.state || snapshot.recovery_id != observed.recovery_id
                {
                    let recovered = snapshot.state == ConnectionState::Connected
                        && snapshot.recovery_id != observed.recovery_id;
                    let terminal = snapshot.state == ConnectionState::Disconnected;
                    let message = api
                        .last_error()
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| agentkit_ui::t!("ak.reconnecting"));
                    if weak
                        .update_in(cx, |this, _, cx| {
                            this.app_state.update(cx, |s, cx| {
                                s.connected = snapshot.state == ConnectionState::Connected;
                                cx.notify();
                            });
                            let message = this
                                .runtime
                                .borrow()
                                .as_ref()
                                .and_then(|r| r.sidecar.as_ref())
                                .and_then(SidecarHandle::last_error)
                                .map(|e| sidecar_message(&e))
                                .unwrap_or(message);
                            if recovered {
                                let reload = snapshot.refresh_epoch > this.transport_epoch;
                                if !reload {
                                    this.transport_epoch = snapshot.recovery_id;
                                }
                                this.view
                                    .update(cx, |v, cx| v.transport_recovered(reload, cx));
                            } else if snapshot.state != ConnectionState::Connected {
                                this.view
                                    .update(cx, |v, cx| v.show_error(message, true, cx));
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                    observed = snapshot;
                    if terminal {
                        break;
                    }
                }
                if weak.upgrade().is_none() {
                    break;
                }
            }
        }));
    }
    fn route(
        &mut self,
        events: Vec<(DecodedSessionEvent, ReductionContext)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.operations.retain(|t| !t.is_ready());
        for (event, _) in &events {
            if let Some(agentkit_protocol::generated::SessionEvent::PermissionRequestEvent(e)) =
                event.known()
                && self.approval_notifications.should_send(
                    &e.session_id,
                    &e.request.request_id,
                    window.is_window_active(),
                    self.preferences.value.borrow().approval_notifications,
                )
            {
                let name = self.brand.app_name.clone();
                let bundle = self.brand.bundle_id.clone();
                let body = agentkit_ui::t!("ak.approval.notification");
                let worker = cx.background_executor().spawn(async move {
                    agentkit_capabilities::notifications::approval(&name, &bundle, &body)
                });
                self.operations
                    .push(cx.spawn_in(window, async move |weak, cx| {
                        if worker.await.is_err() {
                            let _ = weak.update_in(cx, |this, _, cx| {
                                this.view.update(cx, |v, cx| v.notification_unavailable(cx));
                            });
                        }
                    }));
            }
        }
        if self.list_refresh.pending() {
            for (event, _) in &events {
                if self.list_backlog.len() < 4096 {
                    self.list_backlog.push(event.clone());
                } else {
                    self.list_overflow = true;
                }
            }
        }
        self.list.update(cx, |list, cx| {
            list.apply_batch(events.iter().map(|(event, _)| event), cx)
        });

        let mut groups: std::collections::HashMap<String, Vec<_>> =
            std::collections::HashMap::new();
        for (event, context) in events {
            let Some(id) = event
                .raw()
                .get("sessionId")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
            else {
                continue;
            };
            if self.loading && self.workspace_switching {
                if self.backlog.len() < 4096 {
                    self.backlog.push((event.clone(), context.clone()));
                } else {
                    self.backlog_overflow = true;
                }
            }
            if matches!(
                event.known(),
                Some(agentkit_protocol::generated::SessionEvent::SessionDeletedEvent(_))
            ) {
                self.stores.remove(&id);
                self.drafts.remove(&id);
                if self
                    .state
                    .as_ref()
                    .is_some_and(|s| s.read(cx).model.state.session.id == id)
                {
                    self.state = None;
                    if let Some(r) = self.runtime.borrow_mut().as_mut() {
                        r.session_id = None;
                    }
                    self.view.update(cx, |v, cx| {
                        v.clear_session(cx);
                        v.show_error(agentkit_ui::t!("ak.session_deleted"), false, cx);
                    });
                }
                continue;
            }
            if self.stores.contains_key(&id) {
                groups.entry(id).or_default().push((event, context));
            } else if self.loading && !self.workspace_switching {
                if self.backlog.len() < 4096 {
                    self.backlog.push((event, context));
                } else {
                    self.backlog_overflow = true;
                }
            }
        }
        for (id, batch) in groups {
            let Some(store) = self.stores.get(&id).cloned() else {
                continue;
            };
            let effects = store.update(cx, |s, cx| s.apply_batch(&batch, cx));
            for effect in effects {
                if let Effect::PermissionModeChanged {
                    permission_mode,
                    mode_version,
                    ..
                } = &effect
                {
                    store.update(cx, |s, cx| {
                        if s.approval.mode_changed(
                            *mode_version,
                            *permission_mode,
                            s.annotations.current(),
                        ) {
                            cx.emit(
                                agentkit_store::core::session::SessionStoreEvent::TurnsChanged {
                                    from_turn: 0,
                                },
                            );
                            cx.notify();
                        }
                    });
                }
                if self
                    .state
                    .as_ref()
                    .is_some_and(|s| s.entity_id() == store.entity_id())
                {
                    match effect {
                        Effect::RestoreInput { text } => self
                            .view
                            .update(cx, |v, cx| v.restore_draft(&text, window, cx)),
                        Effect::ToastError { message } => self.view.update(cx, |v, cx| {
                            v.show_error(
                                if message == "ak.event_invalid" {
                                    agentkit_ui::t!("ak.event_invalid")
                                } else {
                                    message
                                },
                                false,
                                cx,
                            )
                        }),
                        _ => {}
                    }
                }
            }
        }
        let count = running_background_tasks(
            self.stores
                .values()
                .map(|s| &s.read(cx).model.state.session),
        );
        if self.app_state.read(cx).background_tasks != count {
            self.app_state.update(cx, |s, cx| {
                s.background_tasks = count;
                cx.notify();
            });
        }
        if self.list.read(cx).model.refresh_required
            && !self.list_refresh.pending()
            && !self.workspace_switching
        {
            let Some(lease) = self.list_refresh.begin() else {
                return;
            };
            self.list_backlog.clear();
            self.list_overflow = false;
            let generation = self.epoch;
            let refresh_workspace = self.workspace.as_ref().map(|w| w.id.clone());
            if let Some(api) = self.api() {
                self.operations
                    .push(cx.spawn_in(window, async move |weak, cx| {
                        let sessions = api.sessions_generated().await;
                        let _ = weak.update_in(cx, |this, window, cx| {
                            if this.epoch != generation
                                || this.workspace_switching
                                || this.workspace.as_ref().map(|w| &w.id)
                                    != refresh_workspace.as_ref()
                            {
                                return;
                            }
                            if !this.list_refresh.finish(lease) {
                                return;
                            }
                            let fetched = sessions.is_ok();
                            match sessions {
                                Ok(sessions) => {
                                    let replay = std::mem::take(&mut this.list_backlog);
                                    let overflow = std::mem::take(&mut this.list_overflow);
                                    this.list.update(cx, |s, cx| {
                                        s.replace(sessions, cx);
                                        for event in &replay {
                                            s.model.apply(event);
                                        }
                                        s.model.refresh_required |= overflow;
                                    });
                                }
                                Err(error) => this
                                    .view
                                    .update(cx, |v, cx| v.show_error(error.to_string(), false, cx)),
                            }
                            if fetched && this.list.read(cx).model.refresh_required {
                                this.route(Vec::new(), window, cx);
                            }
                        });
                    }));
            }
        }
    }

    fn remember_session(&self, cx: &App) {
        if let Some(store) = &self.state {
            let id = store.read(cx).model.state.session.id.clone();
            let workspace = self.workspace.as_ref().map(|w| w.id.clone());
            self.preferences.update(|p| {
                p.session = Some(id);
                p.workspace = workspace;
            });
        }
    }
    fn mark_viewed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(api), Some(store), Some(workspace)) =
            (self.api(), self.state.clone(), self.workspace.as_ref())
        else {
            return;
        };
        let Ok((api, _)) = api.bind_connection() else {
            return;
        };
        let id = store.read(cx).model.state.session.id.clone();
        let workspace = workspace.id.clone();
        let epoch = self.epoch;
        self.viewing = Some(cx.spawn_in(window, async move |weak, cx| {
            let result = api.view_session(&id, &workspace).await;
            if result.is_ok() {
                let _ = weak.update_in(cx, |this, _, cx| {
                    if this.epoch == epoch {
                        this.list.update(cx, |s, cx| {
                            if let Some(s) = s.model.sessions.iter_mut().find(|s| s.id == id) {
                                s.has_unread = Some(false);
                            }
                            cx.notify();
                        });
                    }
                });
            }
            if let Err(error) = result {
                let _ = weak.update_in(cx, |this, _, cx| {
                    if this.epoch == epoch
                        && this
                            .state
                            .as_ref()
                            .is_some_and(|s| s.entity_id() == store.entity_id())
                    {
                        this.view
                            .update(cx, |v, cx| v.show_error(error.to_string(), false, cx));
                    }
                });
            }
        }));
    }
    fn api(&self) -> Option<WorkbenchApi> {
        self.runtime
            .borrow()
            .as_ref()
            .map(|running| running.api.clone())
    }
    fn save_draft(&mut self, cx: &mut Context<Self>) {
        if let Some((id, text)) = self.view.read(cx).current_draft(cx)
            && let Some(store) = self.stores.get(&id)
        {
            store.update(cx, |s, _| s.model.set_draft(text));
        }
    }
    fn handle(&mut self, intent: WorkbenchIntent, window: &mut Window, cx: &mut Context<Self>) {
        if self.quitting.get() {
            return;
        }
        if let WorkbenchIntent::SendKeyChanged(key) = intent {
            self.preferences.update(|p| p.send_key = key);
            return;
        }
        match &intent {
            WorkbenchIntent::LayoutChanged(shell) => {
                self.preferences.update(|p| p.shell = shell.clone());
                return;
            }
            WorkbenchIntent::CycleAppearance => {
                self.preferences.update(|p| {
                    p.appearance = match p.appearance {
                        Appearance::System => Appearance::Light,
                        Appearance::Light => Appearance::Dark,
                        Appearance::Dark => Appearance::System,
                    }
                });
                let appearance = self.preferences.value.borrow().appearance;
                match appearance {
                    Appearance::System => {
                        gpui_kit::component::Theme::sync_system_appearance(Some(window), cx)
                    }
                    Appearance::Light => gpui_kit::component::Theme::change(
                        gpui_kit::component::ThemeMode::Light,
                        Some(window),
                        cx,
                    ),
                    Appearance::Dark => gpui_kit::component::Theme::change(
                        gpui_kit::component::ThemeMode::Dark,
                        Some(window),
                        cx,
                    ),
                };
                self.app_state.update(cx, |s, cx| {
                    s.theme = format!("{appearance:?}").to_lowercase();
                    cx.notify();
                });
                return;
            }
            _ => {}
        }
        if let WorkbenchIntent::Chat(action) = &intent
            && !matches!(action, agentkit_ui::transcript::ChatIntent::BranchFrom(_))
        {
            self.handle_chat(action.clone(), window, cx);
            return;
        }
        if let WorkbenchIntent::DraftChanged { session_id, text } = intent {
            if let Some(store) = self.stores.get(&session_id) {
                store.update(cx, |s, _| s.model.set_draft(text));
            }
            return;
        }
        self.operations.retain(|task| !task.is_ready());
        if matches!(intent, WorkbenchIntent::Reconnect) {
            self.start(window, cx);
            return;
        }
        let Some(api) = self.api() else {
            self.view.update(cx, |view, cx| {
                view.show_error(agentkit_ui::t!("ak.connection_unavailable"), true, cx)
            });
            return;
        };
        let Ok((api, snapshot)) = api.bind_connection() else {
            self.view.update(cx, |view, cx| {
                view.show_error(agentkit_ui::t!("ak.connection_unavailable"), true, cx)
            });
            return;
        };
        if !transport_is_current(&snapshot, self.transport_epoch) {
            self.view.update(cx, |view, cx| {
                view.show_error(agentkit_ui::t!("ak.reload_required"), true, cx)
            });
            return;
        }
        let intent_epoch = self.epoch;
        match intent {
            WorkbenchIntent::Sidebar(action) => self.handle_navigation(action, api, window, cx),
            selection @ (WorkbenchIntent::ModelChanged { .. }
            | WorkbenchIntent::ThinkingChanged(_)
            | WorkbenchIntent::ModeChanged(_)) => {
                self.change_composer_selection(selection, api, window, cx);
            }
            intent @ (WorkbenchIntent::Submit(_) | WorkbenchIntent::ResendMessage(_)) => {
                let Some(store) = self.state.clone() else {
                    return;
                };
                let is_resend = matches!(intent, WorkbenchIntent::ResendMessage(_));
                let (text, attachments) = match intent {
                    WorkbenchIntent::Submit(text) => {
                        (text, store.read(cx).model.draft_attachments.clone())
                    }
                    WorkbenchIntent::ResendMessage(id) => {
                        let model = &store.read(cx).model;
                        if model.busy() {
                            return;
                        }
                        let Some(message) = model.state.session.messages.iter().find(|m| {
                            m.id == id && m.role == agentkit_protocol::generated::MessageRole::User
                        }) else {
                            return;
                        };
                        (
                            message.content.clone(),
                            message.attachments.clone().unwrap_or_default(),
                        )
                    }
                    _ => return,
                };
                let context = reduction_context();
                let id = store.read(cx).model.state.session.id.clone();
                let pending = store.update(cx, |s, cx| {
                    let p = if is_resend {
                        s.model
                            .begin_resend(text.clone(), attachments.clone(), &context)
                    } else {
                        s.model
                            .begin_send(text.clone(), attachments.clone(), &context)
                    };
                    if p.is_some() {
                        s.refresh_annotations();
                        cx.emit(
                            agentkit_store::core::session::SessionStoreEvent::TurnsChanged {
                                from_turn: 0,
                            },
                        );
                        cx.notify();
                    }
                    p
                });
                let Some(pending) = pending else {
                    return;
                };
                let generation = self.epoch;
                self.operations.push(cx.spawn_in(window,async move |weak,cx| {
                    let result=api.send_with_attachments(&id,&text,&pending.id,&[],&pending.attachments).await;
                    let _=weak.update_in(cx,|this,window,cx| {
                        if this.epoch!=generation || this.stores.get(&id).is_none_or(|s|s.entity_id()!=store.entity_id()) {return;}
                        let accepted=result.as_ref().is_ok_and(|ack|ack.accepted) || store.read(cx).model.send_confirmed(&pending.id);
                        let clear_draft=accepted && store.read(cx).model.draft_is_current(&pending.id);
                        let failure=(!accepted).then(||result.as_ref().err().map(|e|e.to_string()).unwrap_or_else(||agentkit_ui::t!("ak.send_rejected")));
                        store.update(cx,|s,cx|{s.model.finish_send(&pending.id,accepted);s.model.last_error=failure.clone();if !accepted{cx.emit(agentkit_store::core::session::SessionStoreEvent::TurnsChanged{from_turn:0});}cx.notify();});
                        if this.state.as_ref().is_some_and(|s|s.entity_id()==store.entity_id()) {this.view.update(cx,|v,cx| {v.submission_finished(&text,clear_draft,window,cx);if !accepted {if !is_resend{v.restore_draft(&text,window,cx);}v.show_error(failure.unwrap_or_default(),false,cx);}});}
                    });
                }));
            }
            WorkbenchIntent::Cancel => {
                if let Some(state) = &self.state {
                    let state = state.clone();
                    let id = state.read(cx).model.state.session.id.clone();
                    let epoch = self.epoch;
                    self.operations
                        .push(cx.spawn_in(window, async move |weak, cx| {
                            if let Err(error) = api.cancel(&id).await {
                                let _ = weak.update_in(cx, |this, _, cx| {
                                    if this.epoch != epoch
                                        || this
                                            .state
                                            .as_ref()
                                            .is_none_or(|s| s.entity_id() != state.entity_id())
                                    {
                                        return;
                                    }
                                    this.view.update(cx, |view, cx| {
                                        view.show_error(error.to_string(), false, cx)
                                    })
                                });
                            }
                        }));
                }
            }
            WorkbenchIntent::NewSession
            | WorkbenchIntent::OpenSession(_)
            | WorkbenchIntent::Chat(agentkit_ui::transcript::ChatIntent::BranchFrom(_)) => {
                if self.loading || self.navigation_pending {
                    return;
                }
                self.save_draft(cx);
                let branch = if let WorkbenchIntent::Chat(
                    agentkit_ui::transcript::ChatIntent::BranchFrom(message_id),
                ) = &intent
                {
                    let Some(store) = self.state.as_ref() else {
                        return;
                    };
                    let session = &store.read(cx).model.state.session;
                    if !session.messages.iter().any(|m| {
                        m.id == *message_id
                            && m.role == agentkit_protocol::generated::MessageRole::User
                    }) {
                        return;
                    }
                    Some((
                        session.id.clone(),
                        message_id.clone(),
                        session.working_directory.clone(),
                    ))
                } else {
                    None
                };
                let id = if let WorkbenchIntent::OpenSession(id) = intent {
                    Some(id)
                } else {
                    None
                };
                if let Some(store) = id.as_ref().and_then(|id| self.stores.get(id)).cloned() {
                    self.state = Some(store.clone());
                    if let Some(runtime) = self.runtime.borrow_mut().as_mut() {
                        runtime.session_id = id;
                    }
                    self.view
                        .update(cx, |v, cx| v.bind_session(store, window, cx));
                    self.remember_session(cx);
                    self.mark_viewed(window, cx);
                    return;
                }
                let Some(workspace) = &self.workspace else {
                    return;
                };
                let workspace = workspace.id.clone();
                let working_directory = std::env::var("AGENTKIT_WORKING_DIR").ok().or_else(|| {
                    self.workspace_store
                        .read(cx)
                        .settings
                        .as_ref()
                        .and_then(|s| s.working_directory.clone())
                });
                let generation = self.epoch;
                self.loading = true;
                self.backlog.clear();
                self.backlog_overflow = false;
                self.view.update(cx, |v, cx| v.set_loading(cx));
                self.operations
                    .push(cx.spawn_in(window, async move |weak, cx| {
                        let result = if let Some(id) = id {
                            api.session_generated(&id).await
                        } else if let Some((session, message, directory)) = branch {
                            api.branch_session_generated(
                                &workspace,
                                &session,
                                &message,
                                directory.as_deref(),
                            )
                            .await
                        } else {
                            api.create_session_generated(&workspace, working_directory.as_deref())
                                .await
                        };
                        let summaries = api.sessions_generated().await;
                        let _ = weak.update_in(cx, |this, window, cx| {
                            if !claim_load(this.epoch, generation, &mut this.loading) {
                                return;
                            }
                            if this.backlog_overflow {
                                this.backlog.clear();
                                this.view.update(cx, |v, cx| {
                                    v.show_error(agentkit_ui::t!("ak.load_overflow"), true, cx)
                                });
                                return;
                            }
                            match result {
                                Ok(session) => {
                                    let id = session.id.clone();
                                    let mut store = SessionStore::new(session);
                                    if let Some((draft, attachments)) = this.drafts.remove(&id) {
                                        store.model.draft = draft;
                                        store.model.draft_attachments = attachments;
                                    }
                                    let replay = filter_snapshot_events(
                                        &store.model.state.session,
                                        &this.backlog,
                                    );
                                    for (event, context) in replay {
                                        let _ = store.model.apply(&event, &context);
                                    }
                                    this.backlog.clear();
                                    store.refresh_annotations();
                                    let store = cx.new(|_| store);
                                    this.stores.insert(id.clone(), store.clone());
                                    this.state = Some(store.clone());
                                    if let Some(runtime) = this.runtime.borrow_mut().as_mut() {
                                        runtime.session_id = Some(id);
                                    }
                                    if let Ok(sessions) = summaries {
                                        this.list.update(cx, |s, cx| s.replace(sessions, cx));
                                    }
                                    this.view.update(cx, |v, cx| {
                                        v.bind_session(store, window, cx);
                                    });
                                    this.remember_session(cx);
                                    this.mark_viewed(window, cx);
                                }
                                Err(e) => this
                                    .view
                                    .update(cx, |v, cx| v.show_error(e.to_string(), false, cx)),
                            }
                        });
                    }));
            }
            intent @ WorkbenchIntent::Permission { .. } => {
                self.respond_to_permission(api, intent, window, cx)
            }
            WorkbenchIntent::Configure {
                base_url,
                model,
                credential,
            } => {
                self.operations
                    .push(cx.spawn_in(window, async move |weak, cx| {
                        let result = api
                            .setup_endpoint(&base_url, &model, credential.as_ref())
                            .await;
                        let _ = weak.update_in(cx, |this, window, cx| {
                            if this.epoch != intent_epoch {
                                return;
                            }
                            this.view.update(cx, |view, cx| {
                                view.configuration_finished(result.is_ok(), window, cx);
                                match result {
                                    Ok(()) => view.connected(
                                        this.workspace
                                            .as_ref()
                                            .map(|w| w.name.clone())
                                            .unwrap_or_default(),
                                        Some(model),
                                        cx,
                                    ),
                                    Err(error) => view.show_error(error.to_string(), false, cx),
                                }
                            });
                        });
                    }));
            }
            WorkbenchIntent::Reconnect
            | WorkbenchIntent::DraftChanged { .. }
            | WorkbenchIntent::LayoutChanged(_)
            | WorkbenchIntent::CycleAppearance
            | WorkbenchIntent::SendKeyChanged(_)
            | WorkbenchIntent::Chat(_) => {}
        }
    }
}
impl Render for WorkbenchController {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .track_focus(&self.focus_handle)
            .key_context("Workbench")
            .on_action(cx.listener(|this, _: &ToggleSidebar, window, cx| {
                this.view.update(cx, |v, cx| v.toggle_sidebar(window, cx))
            }))
            .on_action(cx.listener(|this, _: &ToggleRightPanel, window, cx| {
                this.view
                    .update(cx, |v, cx| v.toggle_right_panel(window, cx))
            }))
            .on_action(cx.listener(|this, _: &ToggleFocusMode, window, cx| {
                this.view
                    .update(cx, |v, cx| v.toggle_focus_mode(window, cx))
            }))
            .on_action(cx.listener(|this, _: &StopProcessing, window, cx| {
                if !window.has_active_dialog(cx)
                    && !window.has_active_sheet(cx)
                    && this.state.as_ref().is_some_and(|s| s.read(cx).model.busy())
                {
                    this.handle(WorkbenchIntent::Cancel, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &NewChat, window, cx| {
                this.handle(WorkbenchIntent::NewSession, window, cx)
            }))
            .child(self.view.clone())
    }
}

fn transport_is_current(snapshot: &agentkit_client::ConnectionSnapshot, loaded_epoch: u64) -> bool {
    snapshot.state == ConnectionState::Connected && snapshot.refresh_epoch <= loaded_epoch
}
#[cfg(test)]
mod transport_policy_tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn stale_new_generation_blocks_even_if_monitor_has_not_ticked() {
        let mut snapshot = agentkit_client::ConnectionSnapshot {
            state: ConnectionState::Connected,
            client_id: None,
            server_version: None,
            registered_channels: None,
            max_message_bytes: 1024,
            last_seen_seq: 0,
            recovery_id: 1,
            refresh_epoch: 1,
            needs_refresh: true,
        };
        assert!(transport_is_current(&snapshot, 1));
        snapshot.recovery_id = 2;
        snapshot.needs_refresh = false;
        assert!(
            transport_is_current(&snapshot, 1),
            "unbroken resume keeps loaded history valid"
        );
        snapshot.recovery_id = 3;
        snapshot.refresh_epoch = 3;
        snapshot.needs_refresh = true;
        assert!(!transport_is_current(&snapshot, 1));
        snapshot.recovery_id = 4;
        snapshot.needs_refresh = false;
        assert!(
            !transport_is_current(&snapshot, 1),
            "later resume must not erase missed stale generation"
        );
        assert!(
            transport_is_current(&snapshot, 4),
            "actual reload clears prior debt"
        );
        snapshot.state = ConnectionState::Fatal {
            reason: "auth".into(),
        };
        assert!(!transport_is_current(&snapshot, 1));
    }
}

#[cfg(test)]
mod sidecar_localization_tests {
    #[::core::prelude::v1::test]
    fn terminal_sidecar_errors_use_the_selected_ui_language() {
        agentkit_ui::set_locale("zh-CN");
        let message = super::sidecar_message(&agentkit_sidecar::SidecarError::RestartLimit);
        agentkit_ui::set_locale("en");
        assert_eq!(message, "后端多次退出，已停止自动重启。请重试或查看日志。");
    }
}

fn reduction_context() -> ReductionContext {
    ReductionContext {
        now_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis().min(i64::MAX as u128) as i64),
        message_id: uuid::Uuid::new_v4().to_string(),
    }
}

fn filter_snapshot_events(
    snapshot: &Session,
    events: &[(DecodedSessionEvent, ReductionContext)],
) -> Vec<(DecodedSessionEvent, ReductionContext)> {
    use agentkit_protocol::generated::SessionEvent as E;
    let ids: std::collections::HashSet<_> =
        snapshot.messages.iter().map(|m| m.id.as_str()).collect();
    let mut through = std::collections::HashMap::new();
    for (index, (event, _)) in events.iter().enumerate() {
        if let Some(E::TextCompleteEvent(e)) = event.known()
            && e.session_id == snapshot.id
            && e.message_id.as_deref().is_some_and(|id| ids.contains(id))
        {
            through.insert(e.turn_id.clone(), index);
        }
    }
    events
        .iter()
        .enumerate()
        .filter(|(index, (event, _))| {
            if event
                .raw()
                .get("sessionId")
                .and_then(serde_json::Value::as_str)
                != Some(snapshot.id.as_str())
            {
                return false;
            }
            let turn = match event.known() {
                Some(E::TextDeltaEvent(e)) => Some(&e.turn_id),
                Some(E::TextCompleteEvent(e)) => Some(&e.turn_id),
                _ => None,
            };
            turn.is_none_or(|turn| through.get(turn).is_none_or(|end| *index > *end))
        })
        .map(|(_, event)| event.clone())
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod snapshot_merge_tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn old_load_completion_cannot_unlock_a_new_load() {
        let mut loading = true;
        assert!(!claim_load(2, 1, &mut loading));
        assert!(
            loading,
            "a second open must remain blocked while the current load owns the backlog"
        );
        assert!(claim_load(2, 2, &mut loading));
        assert!(!loading);
    }
    #[::core::prelude::v1::test]
    fn buffered_completed_text_already_in_snapshot_is_not_replayed_but_later_work_is() {
        let snapshot:Session=serde_json::from_value(serde_json::json!({"id":"s","workspaceId":"w","workspaceName":"W","messages":[{"id":"final","role":"assistant","content":"answer","timestamp":4}],"lastMessageAt":4,"isProcessing":false})).unwrap();
        let events:Vec<_>=[serde_json::json!({"type":"text_delta","sessionId":"s","turnId":"old","delta":"part"}),serde_json::json!({"type":"text_complete","sessionId":"s","turnId":"old","messageId":"final","text":"answer"}),serde_json::json!({"type":"text_delta","sessionId":"s","turnId":"new","delta":"next"})].into_iter().map(|v|(serde_json::from_value(v).unwrap(),reduction_context())).collect();
        let remaining = filter_snapshot_events(&snapshot, &events);
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].0.raw()["delta"], "next");
    }
}

fn claim_load(epoch: u64, request: u64, loading: &mut bool) -> bool {
    if epoch != request {
        return false;
    }
    *loading = false;
    true
}

fn bootstrap_preferences(preferences: &Preferences, active: Option<&Session>) -> Preferences {
    let mut selected = preferences.clone();
    // The launch preference must not redirect an explicit runtime reload.
    if let Some(session) = active {
        selected.restore_session = true;
        selected.workspace = Some(session.workspace_id.clone());
        selected.session = Some(session.id.clone());
    }
    selected
}
fn restored_session<'a>(sessions: &'a [Session], preferences: &Preferences) -> Option<&'a Session> {
    if !preferences.restore_session {
        return None;
    }
    sessions
        .iter()
        .find(|s| Some(&s.id) == preferences.session.as_ref())
        .or_else(|| sessions.first())
}
fn running_background_tasks<'a>(sessions: impl IntoIterator<Item = &'a Session>) -> usize {
    use agentkit_protocol::generated::ToolStatus;
    let mut tasks = std::collections::HashSet::new();
    for session in sessions {
        for message in &session.messages {
            if message.is_background == Some(true)
                && !matches!(
                    message.tool_status,
                    Some(ToolStatus::Completed | ToolStatus::Error)
                )
            {
                tasks.insert((
                    session.id.as_str(),
                    message
                        .task_id
                        .as_deref()
                        .or(message.tool_use_id.as_deref())
                        .unwrap_or(&message.id),
                ));
            }
        }
    }
    tasks.len()
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod shell_policy_tests {
    use super::*;
    fn session(id: &str) -> Session {
        serde_json::from_value(serde_json::json!({"id":id,"workspaceId":"w","workspaceName":"W","messages":[],"lastMessageAt":0,"isProcessing":false})).unwrap()
    }
    #[::core::prelude::v1::test]
    fn disabled_restore_creates_empty_instead_of_reopening_recent() {
        let list = vec![session("recent"), session("saved")];
        let mut prefs = Preferences {
            session: Some("saved".into()),
            ..Default::default()
        };
        assert_eq!(restored_session(&list, &prefs).unwrap().id, "saved");
        prefs.session = Some("deleted".into());
        assert_eq!(restored_session(&list, &prefs).unwrap().id, "recent");
        prefs.restore_session = false;
        assert!(restored_session(&list, &prefs).is_none());
    }
    #[::core::prelude::v1::test]
    fn disabled_launch_restore_still_refreshes_active_session_on_reconnect() {
        let prefs = Preferences {
            restore_session: false,
            session: Some("saved".into()),
            ..Default::default()
        };
        let active = session("active");
        let list = vec![session("saved"), active.clone()];
        assert!(restored_session(&list, &bootstrap_preferences(&prefs, None)).is_none());
        let reloaded = bootstrap_preferences(&prefs, Some(&active));
        assert_eq!(restored_session(&list, &reloaded).unwrap().id, "active");
        assert!(!prefs.restore_session);
    }
    #[::core::prelude::v1::test]
    fn background_counts_all_open_sessions_and_deduplicates_task_rows() {
        let mut one = session("one");
        let two = session("two");
        one.messages=serde_json::from_value(serde_json::json!([{"id":"t1","role":"tool","content":"","timestamp":1,"isBackground":true,"taskId":"task","toolStatus":"backgrounded"},{"id":"t2","role":"tool","content":"","timestamp":2,"isBackground":true,"taskId":"task","toolStatus":"backgrounded"}])).unwrap();
        assert_eq!(running_background_tasks([&one, &two]), 1);
        for m in &mut one.messages {
            m.tool_status = Some(agentkit_protocol::generated::ToolStatus::Completed);
        }
        assert_eq!(running_background_tasks([&two, &one]), 0);
    }
}
