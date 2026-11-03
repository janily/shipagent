#[cfg(feature = "perf")]
#[path = "perf.rs"]
mod perf;
#[cfg(feature = "perf")]
pub(crate) fn validate_perf() -> Result<()> {
    perf::Config::from_env().map(|_| ())
}
use crate::Brand;
use agentkit_client::{ConnectionState, Endpoint, api::WorkbenchApi};
use agentkit_protocol::prototype::{Session, SessionEvent, Workspace};
use agentkit_sidecar::{SidecarConfig, SidecarEvent, SidecarHandle};
use agentkit_store::ChatState;
use agentkit_ui::{WorkbenchIntent, WorkbenchView};
use anyhow::{Context as _, Result};
use gpui_kit::*;
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    time::Duration,
};

fn sidecar_message(error: &agentkit_sidecar::SidecarError) -> String {
    use agentkit_sidecar::SidecarError as E;
    let key = match error {
        E::MissingRuntime(_) => "ak.sidecar_missing_runtime",
        E::Io(_) => "ak.sidecar_io",
        E::ReadyTimeout => "ak.sidecar_ready_timeout",
        E::ExitedEarly { .. } => "ak.sidecar_exited_early",
        E::AlreadyRunning { .. } => "ak.sidecar_occupied",
        E::InvalidEndpoint => "ak.sidecar_invalid_endpoint",
        E::RestartLimit => "ak.sidecar_restart_limit",
        E::Stopped => "ak.sidecar_stopped",
    };
    agentkit_ui::tr(key)
}
pub(crate) type StartupSlot = Rc<RefCell<Option<Task<()>>>>;
pub(crate) type RuntimeSlot = Rc<RefCell<Option<Running>>>;
pub(crate) struct Running {
    api: WorkbenchApi,
    sidecar: Option<SidecarHandle>,
    lifecycle: Option<smol::Task<()>>,
    session_id: Option<String>,
}
impl Running {
    pub(crate) async fn shutdown_for_exit(mut self) {
        self.lifecycle.take();
        // GPUI allows only 200 ms in on_app_quit. Close transport first and
        // reserve time to kill and reap the owned process group.
        self.api.close();
        if let Some(sidecar) = self.sidecar {
            let _ = sidecar.shutdown(Duration::from_millis(60)).await;
        }
    }
    pub(crate) async fn shutdown(mut self) {
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
struct Started {
    running: Running,
    workspace: Workspace,
    sessions: Vec<Session>,
    session: Session,
    model: Option<String>,
    events: async_channel::Receiver<Vec<serde_json::Value>>,
}
async fn bootstrap(brand: Brand) -> Result<Started> {
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
    match initialize(endpoint).await {
        Ok(mut started) => {
            if let Some(handle) = &sidecar {
                let events = handle.events();
                let api = started.running.api.clone();
                started.running.lifecycle = Some(smol::spawn(async move {
                    while let Ok(event) = events.recv().await {
                        match event {
                            SidecarEvent::Restarted { endpoint } => api.replace_endpoint(endpoint),
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
async fn initialize(endpoint: Endpoint) -> Result<Started> {
    let initial = WorkbenchApi::connect(endpoint.clone(), String::new());
    let workspaces = initial.workspaces().await?;
    let workspace = workspaces
        .into_iter()
        .next()
        .context("No backend workspace available")?;
    initial.disable_browser().await?;
    let connections = initial.connections().await?;
    let model = connections
        .iter()
        .find(|c| c.is_authenticated && c.is_default)
        .or_else(|| connections.iter().find(|c| c.is_authenticated))
        .map(|c| c.default_model.clone().unwrap_or_else(|| c.name.clone()));
    initial.close();
    let api = WorkbenchApi::connect(endpoint, workspace.id.clone());
    let events = api.session_events();
    let mut sessions = api.sessions().await?;
    let session = if let Some(summary) = sessions.first() {
        api.session(&summary.id).await?
    } else {
        let working_directory = std::env::var("AGENTKIT_WORKING_DIR").ok();
        let session = api
            .create_session(&workspace.id, working_directory.as_deref())
            .await?;
        sessions.push(session.clone());
        session
    };
    let running = Running {
        api,
        sidecar: None,
        lifecycle: None,
        session_id: Some(session.id.clone()),
    };
    Ok(Started {
        running,
        workspace,
        sessions,
        session,
        model,
        events,
    })
}

pub(crate) struct WorkbenchController {
    #[cfg(feature = "perf")]
    perf: Option<perf::Capture>,
    view: Entity<WorkbenchView>,
    brand: Brand,
    runtime: RuntimeSlot,
    workspace: Option<Workspace>,
    state: Option<ChatState>,
    quitting: Rc<Cell<bool>>,
    loading: bool,
    transport_epoch: u64,
    startup: StartupSlot,
    events: Option<Task<()>>,
    monitor: Option<Task<()>>,
    operations: Vec<Task<()>>,
    _subscription: Subscription,
}
impl WorkbenchController {
    pub(crate) fn new(
        brand: Brand,
        runtime: RuntimeSlot,
        startup: StartupSlot,
        quitting: Rc<Cell<bool>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let view = cx.new(|cx| WorkbenchView::new(brand.app_name.clone().into(), window, cx));
        let subscription = cx.subscribe_in(&view, window, |this, _, intent, window, cx| {
            this.handle(intent.clone(), window, cx)
        });
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
            state: None,
            quitting,
            loading: false,
            transport_epoch: 0,
            startup,
            events: None,
            monitor: None,
            operations: vec![],
            _subscription: subscription,
        };
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
        self.events = None;
        self.monitor = None;
        let previous = self.runtime.borrow_mut().take();
        let brand = self.brand.clone();
        let quitting = self.quitting.clone();
        *self.startup.borrow_mut() = Some(cx.spawn_in(window, async move |weak, cx| {
            if let Some(previous) = previous {
                previous.shutdown().await;
            }
            // Environment and filesystem work stays off the GPUI foreground thread.
            let result = cx
                .background_executor()
                .spawn(async move { bootstrap(brand).await })
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
        let state = ChatState::new(started.session);
        self.workspace = Some(started.workspace.clone());
        self.state = Some(state.clone());
        let api = started.running.api.clone();
        self.transport_epoch = api.snapshot().recovery_id;
        *self.runtime.borrow_mut() = Some(started.running);
        self.view.update(cx, |view, cx| {
            view.connected(started.workspace.name, started.model, cx);
            view.set_sessions(started.sessions, cx);
            view.show_session(state, cx);
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
                while batch.len() < 256 {
                    match events.try_recv() {
                        Ok(event) => batch.push(event),
                        Err(_) => break,
                    }
                }
                if weak
                    .update_in(cx, |this, _window, cx| {
                        #[cfg(feature = "perf")]
                        let update_started = std::time::Instant::now();
                        if let Some(state) = &mut this.state {
                            for args in batch {
                                if let Some(value) = args.into_iter().next()
                                    && let Ok(event) = serde_json::from_value::<SessionEvent>(value)
                                {
                                    #[cfg(feature = "perf")]
                                    if let Some(capture) = &mut this.perf {
                                        capture.observe(&event);
                                    }
                                    state.apply(&event);
                                }
                            }
                            let snapshot = state.clone();
                            if !this.loading {
                                this.view
                                    .update(cx, |view, cx| view.show_session(snapshot, cx));
                            }
                        }
                        #[cfg(feature = "perf")]
                        if let Some(capture) = &mut this.perf {
                            capture.note_update(
                                received,
                                update_started,
                                _window.is_window_active(),
                            );
                        }
                        #[cfg(feature = "perf")]
                        perf::finish_if_terminal(this, _window, cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
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
                        .unwrap_or_else(|| "Backend reconnecting".into());
                    if weak
                        .update_in(cx, |this, _, cx| {
                            let message = this
                                .runtime
                                .borrow()
                                .as_ref()
                                .and_then(|runtime| runtime.sidecar.as_ref())
                                .and_then(SidecarHandle::last_error)
                                .map(|error| sidecar_message(&error))
                                .unwrap_or(message);
                            if recovered {
                                let requires_reload = snapshot.refresh_epoch > this.transport_epoch;
                                if !requires_reload {
                                    this.transport_epoch = snapshot.recovery_id;
                                }
                                this.view.update(cx, |view, cx| {
                                    view.transport_recovered(requires_reload, cx)
                                });
                            } else if snapshot.state != ConnectionState::Connected {
                                this.view
                                    .update(cx, |view, cx| view.show_error(message, true, cx));
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

    fn api(&self) -> Option<WorkbenchApi> {
        self.runtime
            .borrow()
            .as_ref()
            .map(|running| running.api.clone())
    }
    fn handle(&mut self, intent: WorkbenchIntent, window: &mut Window, cx: &mut Context<Self>) {
        if self.quitting.get() {
            return;
        }
        self.operations.retain(|task| !task.is_ready());
        if matches!(intent, WorkbenchIntent::Reconnect) {
            self.start(window, cx);
            return;
        }
        let Some(api) = self.api() else {
            return;
        };
        let Ok((api, snapshot)) = api.bind_connection() else {
            self.view.update(cx, |view, cx| {
                view.show_error("连接不可用，草稿已保留。".into(), true, cx)
            });
            return;
        };
        if !transport_is_current(&snapshot, self.transport_epoch) {
            self.view.update(cx, |view, cx| {
                view.show_error(
                    "连接不可用或状态需要重新加载，草稿已保留。".into(),
                    true,
                    cx,
                )
            });
            return;
        }
        match intent {
            WorkbenchIntent::Submit(text) => {
                let Some(state) = &mut self.state else {
                    return;
                };
                let optimistic = uuid::Uuid::new_v4().to_string();
                if !state.begin_send(text.clone(), optimistic.clone()) {
                    return;
                }
                let id = state.session.id.clone();
                let snapshot = state.clone();
                self.view
                    .update(cx, |view, cx| view.show_session(snapshot, cx));
                self.operations
                    .push(cx.spawn_in(window, async move |weak, cx| {
                        let result = api.send(&id, &text, &optimistic).await;
                        let _ = weak.update_in(cx, |this, window, cx| {
                            if this
                                .state
                                .as_ref()
                                .is_none_or(|state| state.session.id != id)
                            {
                                return;
                            }
                            if let Err(error) = &result
                                && let Some(state) = &mut this.state
                            {
                                state.fail_send(&optimistic, error.to_string());
                                let snapshot = state.clone();
                                this.view
                                    .update(cx, |view, cx| view.show_session(snapshot, cx));
                            }
                            this.view.update(cx, |view, cx| {
                                view.submission_finished(&text, result.is_ok(), window, cx)
                            });
                        });
                    }));
            }
            WorkbenchIntent::Cancel => {
                if let Some(state) = &self.state {
                    let id = state.session.id.clone();
                    self.operations
                        .push(cx.spawn_in(window, async move |weak, cx| {
                            if let Err(error) = api.cancel(&id).await {
                                let _ = weak.update_in(cx, |this, _, cx| {
                                    this.view.update(cx, |view, cx| {
                                        view.show_error(error.to_string(), false, cx)
                                    })
                                });
                            }
                        }));
                }
            }
            WorkbenchIntent::NewSession | WorkbenchIntent::OpenSession(_) => {
                let Some(workspace) = &self.workspace else {
                    return;
                };
                let workspace = workspace.id.clone();
                if self.loading {
                    return;
                }
                self.loading = true;
                self.view.update(cx, |view, cx| view.set_loading(cx));
                let id = if let WorkbenchIntent::OpenSession(id) = intent {
                    Some(id)
                } else {
                    None
                };
                self.operations
                    .push(cx.spawn_in(window, async move |weak, cx| {
                        let result = if let Some(id) = id {
                            api.session(&id).await
                        } else {
                            let directory = std::env::var("AGENTKIT_WORKING_DIR").ok();
                            api.create_session(&workspace, directory.as_deref()).await
                        };
                        let summaries = api.sessions().await;
                        let _ = weak.update_in(cx, |this, _, cx| {
                            this.loading = false;
                            match result {
                                Ok(session) => {
                                    if let Some(runtime) = this.runtime.borrow_mut().as_mut() {
                                        runtime.session_id = Some(session.id.clone());
                                    }
                                    this.state = Some(ChatState::new(session));
                                    if let Some(state) = &this.state {
                                        this.view.update(cx, |view, cx| {
                                            if let Ok(sessions) = summaries {
                                                view.set_sessions(sessions, cx);
                                            }
                                            view.show_session(state.clone(), cx);
                                        });
                                    }
                                }
                                Err(error) => this.view.update(cx, |view, cx| {
                                    if let Some(state) = &this.state {
                                        view.show_session(state.clone(), cx);
                                    }
                                    view.show_error(error.to_string(), false, cx)
                                }),
                            }
                        });
                    }));
            }
            WorkbenchIntent::Permission {
                request_id,
                allowed,
                always,
            } => {
                let Some(state) = &self.state else {
                    return;
                };
                let id = state.session.id.clone();
                self.operations
                    .push(cx.spawn_in(window, async move |weak, cx| {
                        let result = api
                            .respond_permission(&id, &request_id, allowed, always)
                            .await;
                        let _ = weak.update_in(cx, |this, _, cx| match result {
                            Ok(()) => {
                                if let Some(state) = &mut this.state
                                    && state.session.id == id
                                {
                                    state.permission_responded(&request_id);
                                    let snapshot = state.clone();
                                    this.view
                                        .update(cx, |view, cx| view.show_session(snapshot, cx));
                                }
                            }
                            Err(error) => this.view.update(cx, |view, cx| {
                                view.show_error(error.to_string(), false, cx)
                            }),
                        });
                    }));
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
            WorkbenchIntent::Reconnect => {}
        }
    }
}
impl Render for WorkbenchController {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.view.clone()
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
    #[test]
    fn terminal_sidecar_errors_use_the_selected_ui_language() {
        agentkit_ui::set_locale("zh-CN");
        let message = super::sidecar_message(&agentkit_sidecar::SidecarError::RestartLimit);
        agentkit_ui::set_locale("en");
        assert_eq!(message, "后端多次退出，已停止自动重启。请重试或查看日志。");
    }
}
