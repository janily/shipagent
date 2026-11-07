//! Opt-in full-application E2E driver. Only normal UI intents and real RPC are used.
use super::*;
use agentkit_protocol::generated::{MessageRole, PermissionMode};
use agentkit_ui::{onboarding::OnboardingIntent, settings::ConnectionKind};
use gpui_kit::profiler::{FrameEvent, FrameTimingCollector};
use secrecy::{ExposeSecret, SecretString};
use std::{collections::BTreeMap, fs, sync::OnceLock, time::Instant};
static CONFIG: OnceLock<Config> = OnceLock::new();
#[derive(Clone)]
struct Config {
    root: PathBuf,
    model: String,
    case: String,
}
pub(crate) fn configure() -> Result<()> {
    let Some(root) = std::env::var_os("AGENTKIT_E2E_ROOT") else {
        return Ok(());
    };
    let root = PathBuf::from(root).canonicalize()?;
    let case = std::env::var("AGENTKIT_E2E_CASE")?;
    anyhow::ensure!(
        ["full", "restore", "defer", "defer_restore"].contains(&case.as_str()),
        "Invalid E2E case"
    );
    anyhow::ensure!(
        fs::read_to_string(root.join(".shipagent-e2e"))? == "shipagent-e2e-v1",
        "Private E2E marker required"
    );
    for (key, leaf) in [
        ("AGENTKIT_CONFIG_DIR", "data"),
        ("AGENTKIT_APP_DATA_DIR", "client"),
        ("AGENTKIT_WORKING_DIR", "fixture"),
    ] {
        let path = std::env::var_os(key)
            .map(PathBuf::from)
            .ok_or_else(|| anyhow::anyhow!("Isolated E2E paths required"))?;
        let parent = path.parent().and_then(|p| p.canonicalize().ok());
        let valid_child = match path.symlink_metadata() {
            Ok(_) => path.canonicalize().is_ok_and(|p| p == root.join(leaf)),
            Err(error) => error.kind() == std::io::ErrorKind::NotFound,
        };
        anyhow::ensure!(
            parent.as_ref() == Some(&root)
                && path.file_name() == Some(std::ffi::OsStr::new(leaf))
                && valid_child,
            "Isolated E2E paths required"
        );
    }

    anyhow::ensure!(
        std::env::var_os("AGENTKIT_REMOTE_URL").is_none()
            && std::env::var_os("AGENTKIT_REMOTE_TOKEN").is_none()
            && std::env::var_os("AGENTKIT_PERF_ROOT").is_none(),
        "E2E cannot attach remotely or bypass onboarding"
    );
    if matches!(case.as_str(), "full" | "defer") {
        anyhow::ensure!(
            !root.join("data").exists() || fs::read_dir(root.join("data"))?.next().is_none(),
            "Fresh E2E profile required"
        );
    }
    anyhow::ensure!(
        !root.join(format!("{case}.json")).exists(),
        "E2E output already exists"
    );
    let model = std::env::var("AGENTKIT_E2E_MODEL_URL")?;
    let url = url::Url::parse(&model)?;
    anyhow::ensure!(
        url.scheme() == "http"
            && url.host_str() == Some("127.0.0.1")
            && url.port().is_some_and(|p| p > 0)
            && url.path() == "/v1"
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "E2E model must be owned loopback HTTP"
    );
    CONFIG
        .set(Config { root, model, case })
        .map_err(|_| anyhow::anyhow!("E2E already configured"))?;
    Ok(())
}
pub(crate) async fn record_shutdown(passed: bool) {
    let Some(c) = CONFIG.get().cloned() else {
        return;
    };
    let _ = smol::unblock(move || -> std::io::Result<()> {
        let f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(c.root.join(format!("{}-shutdown.json", c.case)))?;
        serde_json::to_writer(f, &serde_json::json!({"shutdownPassed":passed}))
            .map_err(std::io::Error::other)
    })
    .await;
}
pub(crate) struct Probe {
    config: Config,
    phase: u8,
    started: Instant,
    frames: FrameTimingCollector,
    presented: usize,
    checks: BTreeMap<String, bool>,
    events: BTreeMap<String, u64>,
    active: String,
    pending: String,
    optimistic: bool,
    ack: bool,
    initial: Option<String>,
    canonical: BTreeMap<String, String>,
    history: Vec<(String, MessageRole, String)>,
    old_epoch: u64,
    token: Option<SecretString>,
    fault_at: Option<Instant>,
    cancel_at: Option<Instant>,
    timings: BTreeMap<String, f64>,
}
impl Probe {
    fn check(&mut self, key: &str, value: bool) {
        self.checks.insert(key.into(), value);
    }
    fn count(&self, key: &str) -> u64 {
        self.events.get(key).copied().unwrap_or(0)
    }
    pub(super) fn observe(&mut self, value: &serde_json::Value) {
        if value.get("sessionId").and_then(|v| v.as_str()) != Some(self.active.as_str()) {
            return;
        }
        if value.get("type").and_then(|v| v.as_str()) == Some("user_message")
            && let (Some(local), Some(server)) = (
                value.get("optimisticMessageId").and_then(|v| v.as_str()),
                value
                    .get("message")
                    .and_then(|v| v.get("id"))
                    .and_then(|v| v.as_str()),
            )
        {
            self.canonical.insert(local.into(), server.into());
        }
        if let Some(kind) = value.get("type").and_then(|v| v.as_str())
            && [
                "user_message",
                "text_delta",
                "tool_start",
                "tool_result",
                "permission_request",
                "complete",
                "interrupted",
                "error",
                "typed_error",
            ]
            .contains(&kind)
        {
            *self.events.entry(kind.into()).or_default() += 1;
        }
    }
    pub(super) fn optimistic(
        &mut self,
        id: &str,
        pending: &str,
        session: &agentkit_protocol::generated::Session,
    ) {
        if id == self.active {
            self.pending = pending.into();
            self.optimistic = session.is_processing
                && session
                    .messages
                    .iter()
                    .filter(|m| m.id == pending && m.is_pending == Some(true))
                    .count()
                    == 1;
        }
    }
    pub(super) fn ack(&mut self, id: &str, accepted: bool) {
        if id == self.active {
            self.ack = accepted;
        }
    }
    fn send(
        &mut self,
        this: &mut WorkbenchController,
        text: &str,
        window: &mut Window,
        cx: &mut Context<WorkbenchController>,
    ) -> bool {
        if !this.view.read(cx).e2e_composer_ready(cx) {
            return false;
        }
        self.events.clear();
        self.optimistic = false;
        self.ack = false;
        self.pending.clear();
        self.active = this
            .state
            .as_ref()
            .map(|s| s.read(cx).model.state.session.id.clone())
            .unwrap_or_default();
        this.view.update(cx, |v, cx| v.e2e_send(text, window, cx));
        true
    }
    fn emit(
        this: &mut WorkbenchController,
        intent: WorkbenchIntent,
        cx: &mut Context<WorkbenchController>,
    ) {
        this.view.update(cx, |_, cx| cx.emit(intent));
    }
    fn decision(
        &self,
        this: &mut WorkbenchController,
        allowed: bool,
        always: bool,
        cx: &mut Context<WorkbenchController>,
    ) -> bool {
        let request = this
            .state
            .as_ref()
            .and_then(|s| s.read(cx).model.permissions.first().cloned());
        if let Some(r) = request {
            if always && r.can_always_allow != Some(true) {
                return false;
            }
            Self::emit(
                this,
                WorkbenchIntent::Permission {
                    session_id: r.session_id,
                    request_id: r.request_id,
                    allowed,
                    always,
                },
                cx,
            );
            true
        } else {
            false
        }
    }
    fn tick(
        &mut self,
        this: &mut WorkbenchController,
        window: &mut Window,
        cx: &mut Context<WorkbenchController>,
    ) -> Option<&'static str> {
        self.presented += self.frames.collect_unseen().iter().filter(|e|matches!(e,FrameEvent::Present(p) if p.window_id==window.window_handle().window_id())).count();
        if self.started.elapsed() > Duration::from_secs(55) {
            return Some("scenario-deadline");
        }
        if self.presented == 0 {
            return None;
        }
        let ready = this.view.read(cx).connection_ready() && !this.recovery.pending;
        let wizard = this.view.read(cx).onboarding();
        let editor = this.settings_editor.clone();
        if self.config.case == "restore" && ready && wizard.is_none() {
            self.check(
                "restore.authenticated",
                this.app_state
                    .read(cx)
                    .connections
                    .iter()
                    .any(|c| c.is_authenticated && c.is_default == Some(true)),
            );
            self.check(
                "restore.session",
                this.state.as_ref().is_some_and(|s| {
                    let session = &s.read(cx).model.state.session;
                    Some(&session.id) == self.initial.as_ref()
                        && [
                            "[e2e:read]",
                            "[e2e:deny]",
                            "[e2e:allow]",
                            "[e2e:always]",
                            "[e2e:cancel]",
                            "[e2e:crash]",
                            "[e2e:continue]",
                        ]
                        .iter()
                        .all(|text| {
                            session
                                .messages
                                .iter()
                                .filter(|m| m.role == MessageRole::User && m.content == *text)
                                .count()
                                == 1
                        })
                        && ["READ_OK", "DENY_OK", "ALLOW_OK", "ALWAYS_OK", "CONTINUE_OK"]
                            .iter()
                            .all(|text| {
                                session.messages.iter().any(|m| {
                                    m.role == MessageRole::Assistant && m.content.contains(text)
                                })
                            })
                }),
            );
            return Some("done");
        }
        if self.config.case == "defer_restore" && ready && wizard.is_none() {
            self.check(
                "defer.restored",
                this.preferences.value.borrow().onboarding_deferred,
            );
            return Some("done");
        }
        if self.config.case == "defer" {
            if self.phase == 0
                && let Some(wizard) = wizard
                && !this.settings_pending
            {
                self.check("defer.wizard", true);
                wizard.update(cx, |_, cx| cx.emit(OnboardingIntent::Defer));
                self.phase = 1;
            } else if self.phase == 1 && this.view.read(cx).onboarding().is_none() {
                self.check(
                    "defer.acknowledged",
                    this.preferences.value.borrow().onboarding_deferred,
                );
                return Some("done");
            }
            return None;
        }
        let session = this
            .state
            .as_ref()
            .map(|s| s.read(cx).model.state.session.clone());
        let idle = session.as_ref().is_some_and(|s| !s.is_processing) && self.count("complete") > 0;
        let permissions = this
            .state
            .as_ref()
            .map_or(0, |s| s.read(cx).model.permissions.len());
        match self.phase {
            0 if !this.settings_pending && wizard.is_some() && this.api().is_some() => {
                self.check("u1.onboarding", true);
                self.check(
                    "u1.browser_initialized",
                    this.preferences.value.borrow().browser_initialized,
                );
                if let Some(w) = wizard {
                    w.update(cx, |_, cx| {
                        cx.emit(OnboardingIntent::Choose(ConnectionKind::OpenAi))
                    });
                }
                self.phase = 1;
            }
            1 if !this.settings_pending && editor.is_some() => {
                if let Some(e) = editor {
                    e.update(cx, |e, cx| {
                        e.e2e_endpoint(&format!("{}/reject", self.config.model), window, cx)
                    });
                }
                self.phase = 2;
            }
            2 if !this.settings_pending => {
                if let Some(e) = editor {
                    e.update(cx, |e, cx| e.e2e_submit(false, cx));
                    self.check(
                        "u1.test_gate",
                        this.onboarding_saved.is_none() && !e.read(cx).e2e_state().1,
                    );
                    e.update(cx, |e, cx| e.e2e_submit(true, cx));
                    self.phase = 3;
                }
            }
            3 if !this.settings_pending => {
                if let Some(e) = editor
                    && e.read(cx).e2e_state().2
                {
                    self.check("u1.rejected_test", true);
                    e.update(cx, |e, cx| e.e2e_endpoint(&self.config.model, window, cx));
                    self.phase = 4;
                }
            }
            4 if !this.settings_pending => {
                if let Some(e) = editor {
                    e.update(cx, |e, cx| e.e2e_submit(true, cx));
                    self.phase = 5;
                }
            }
            5 if !this.settings_pending => {
                if editor.as_ref().is_some_and(|e| e.read(cx).e2e_state().2) {
                    return Some("connection-test-failed");
                }
                if let Some(e) = editor
                    && e.read(cx).e2e_state().0
                {
                    self.check("u1.tested", true);
                    e.update(cx, |e, cx| e.e2e_submit(false, cx));
                    self.phase = 6;
                }
            }
            6 if !this.settings_pending => {
                if let Some(w) = wizard
                    && w.read(cx).e2e_step() == 3
                {
                    self.check(
                        "u1.saved_default",
                        this.app_state
                            .read(cx)
                            .connections
                            .iter()
                            .any(|c| c.is_authenticated && c.is_default == Some(true)),
                    );
                    w.update(cx, |_, cx| cx.emit(OnboardingIntent::Finish));
                    self.phase = 7;
                }
            }
            7 if ready && wizard.is_none() => {
                self.check(
                    "u1.focused_empty_session",
                    session.as_ref().is_some_and(|s| s.messages.is_empty())
                        && this
                            .view
                            .read(cx)
                            .composer_input(cx)
                            .read(cx)
                            .focus_handle(cx)
                            .is_focused(window),
                );
                if self.send(this, "[e2e:read]", window, cx) {
                    self.phase = 8;
                }
            }
            8 if idle => {
                let Some(session) = session else {
                    return Some("missing-session");
                };
                let rendered = session
                    .messages
                    .iter()
                    .rev()
                    .find(|m| m.role == MessageRole::Assistant)
                    .and_then(|m| this.view.read(cx).cached_markdown(&m.id, cx));
                if !rendered.is_some_and(|s| s.contains("READ_OK")) {
                    return None;
                }
                self.check("u2.optimistic", self.optimistic);
                self.check("u2.ack", self.ack);
                self.check(
                    "u2.stream",
                    self.count("text_delta") > 0
                        && session
                            .messages
                            .iter()
                            .filter(|m| {
                                m.id == self.pending
                                    && m.role == MessageRole::User
                                    && m.is_pending != Some(true)
                            })
                            .count()
                            == 1,
                );
                self.check(
                    "u2.tools",
                    self.count("tool_start") > 0
                        && self.count("tool_result") > 0
                        && session.messages.iter().any(|m| m.role == MessageRole::Tool),
                );
                let Some(api) = this.api() else {
                    return Some("missing-api");
                };
                let expected = session
                    .messages
                    .iter()
                    .filter(|m| matches!(m.role, MessageRole::User | MessageRole::Assistant))
                    .map(|m| {
                        (
                            self.canonical
                                .get(&m.id)
                                .cloned()
                                .unwrap_or_else(|| m.id.clone()),
                            m.role,
                            m.content.clone(),
                        )
                    })
                    .collect::<Vec<_>>();
                let id = session.id;
                self.phase = 9;
                this.operations
                    .push(cx.spawn_in(window, async move |weak, cx| {
                        let persisted = api.session_generated(&id).await;
                        let matches = persisted.is_ok_and(|s| {
                            s.messages
                                .iter()
                                .filter(|m| {
                                    matches!(m.role, MessageRole::User | MessageRole::Assistant)
                                })
                                .map(|m| (m.id.clone(), m.role, m.content.clone()))
                                .collect::<Vec<_>>()
                                == expected
                        });
                        let _ = weak.update_in(cx, |this, _, _| {
                            if let Some(p) = &mut this.e2e {
                                p.check("u2.persisted", matches);
                            }
                        });
                    }));
            }
            9 if self.checks.contains_key("u2.persisted") => {
                this.view
                    .update(cx, |v, cx| v.cycle_mode_from_menu(window, cx));
                self.phase = 10;
            }
            10 if session
                .as_ref()
                .is_some_and(|s| s.permission_mode == Some(PermissionMode::Ask)) =>
            {
                if self.send(this, "[e2e:deny]", window, cx) {
                    self.phase = 11;
                }
            }
            11 if permissions > 0 => {
                if self.decision(this, false, false, cx) {
                    self.phase = 12;
                }
            }
            12 if idle && permissions == 0 => {
                self.check(
                    "u3.deny",
                    session.as_ref().is_some_and(|s| {
                        s.messages
                            .iter()
                            .any(|m| m.role == MessageRole::Tool && m.is_error == Some(true))
                    }),
                );
                if self.send(this, "[e2e:allow]", window, cx) {
                    self.phase = 13;
                }
            }
            13 if permissions > 0 => {
                if self.decision(this, true, false, cx) {
                    self.phase = 14;
                }
            }
            14 if idle && permissions == 0 => {
                self.check(
                    "u3.allow",
                    self.count("permission_request") == 1 && self.count("tool_result") == 1,
                );
                if self.send(this, "[e2e:always]", window, cx) {
                    self.phase = 15;
                }
            }
            15 if permissions > 0 => {
                if self.decision(this, true, true, cx) {
                    self.phase = 16;
                }
            }
            16 if idle && permissions == 0 => {
                self.check(
                    "u3.always",
                    self.count("permission_request") == 1
                        && self.count("tool_start") == 2
                        && self.count("tool_result") == 2,
                );
                self.check(
                    "u3.receipts",
                    this.state.as_ref().is_some_and(|s| {
                        let r = &s.read(cx).approval.receipts;
                        r.iter().any(|r| !r.allowed)
                            && r.iter().any(|r| r.allowed && !r.always)
                            && r.iter().any(|r| r.allowed && r.always)
                    }),
                );
                self.history = session
                    .as_ref()
                    .map(|s| &s.messages)
                    .into_iter()
                    .flatten()
                    .filter(|m| matches!(m.role, MessageRole::User | MessageRole::Assistant))
                    .map(|m| {
                        (
                            self.canonical
                                .get(&m.id)
                                .cloned()
                                .unwrap_or_else(|| m.id.clone()),
                            m.role,
                            m.content.clone(),
                        )
                    })
                    .collect();
                if self.send(this, "[e2e:cancel]", window, cx) {
                    self.phase = 17;
                }
            }
            17 if self.count("text_delta") >= 2 => {
                Self::emit(this, WorkbenchIntent::Cancel, cx);
                self.cancel_at = Some(Instant::now());
                self.phase = 18;
            }
            18 if session.as_ref().is_some_and(|s| !s.is_processing)
                && self.count("interrupted") > 0 =>
            {
                let elapsed = self
                    .cancel_at
                    .map(|t| t.elapsed().as_secs_f64() * 1000.)
                    .unwrap_or(f64::INFINITY);
                self.timings.insert("cancelMs".into(), elapsed);
                self.check("f17.cancel", elapsed <= 2000.);
                if self.send(this, "[e2e:crash]", window, cx) {
                    self.phase = 19;
                }
            }
            19 if self.count("text_delta") >= 2 => {
                let input = this.view.read(cx).composer_input(cx);
                input.update(cx, |input, cx| input.set_value("E2E_UNSENT", window, cx));
                self.phase = 20;
            }
            20 => {
                let runtime = this.runtime.borrow();
                if let Some(r) = runtime.as_ref()
                    && let Some(sidecar) = r.sidecar.as_ref()
                {
                    self.token = Some(sidecar.endpoint().token);
                    self.old_epoch = this.transport_epoch;
                    self.fault_at = Some(Instant::now());
                    if sidecar.inject_crash_for_e2e().is_err() {
                        return Some("crash-injection-failed");
                    };
                    self.phase = 21;
                }
            }
            21 => {
                if !ready {
                    self.check("u6.disconnected", true);
                }
                if ready && this.transport_epoch > self.old_epoch {
                    self.timings.insert(
                        "recoveryMs".into(),
                        self.fault_at
                            .map(|t| t.elapsed().as_secs_f64() * 1000.)
                            .unwrap_or_default(),
                    );
                    self.check("u6.restart", true);
                    self.check(
                        "u6.token_rotated",
                        this.runtime
                            .borrow()
                            .as_ref()
                            .and_then(|r| r.sidecar.as_ref())
                            .is_some_and(|s| {
                                self.token.as_ref().is_some_and(|old| {
                                    old.expose_secret() != s.endpoint().token.expose_secret()
                                })
                            }),
                    );
                    self.check(
                        "u6.session_refreshed",
                        session.as_ref().is_some_and(|s| {
                            s.id == self.active
                                && !s.is_processing
                                && self.history.iter().all(|(id, role, text)| {
                                    s.messages.iter().any(|m| {
                                        &m.id == id && &m.role == role && &m.content == text
                                    })
                                })
                        }) && this
                            .list
                            .read(cx)
                            .model
                            .sessions
                            .iter()
                            .filter(|s| s.id == self.active)
                            .count()
                            == 1
                            && this
                                .view
                                .read(cx)
                                .current_draft(cx)
                                .is_some_and(|(_, d)| d == "E2E_UNSENT"),
                    );
                    self.check(
                        "u6.interrupted",
                        session.as_ref().is_some_and(|s| {
                            s.messages.iter().any(|m| {
                                m.role == MessageRole::Info
                                    && m.content == "Interrupted. You can continue."
                                    && s.messages
                                        .iter()
                                        .rposition(|u| {
                                            u.role == MessageRole::User
                                                && u.content == "[e2e:crash]"
                                        })
                                        .is_some_and(|i| {
                                            s.messages
                                                .iter()
                                                .position(|x| x.id == m.id)
                                                .is_some_and(|j| j > i)
                                        })
                            })
                        }),
                    );
                    if self.send(this, "[e2e:continue]", window, cx) {
                        self.phase = 22;
                    }
                }
            }
            22 if idle => {
                self.check(
                    "u6.continued",
                    session.is_some_and(|s| {
                        s.messages.iter().any(|m| {
                            m.role == MessageRole::Assistant && m.content.contains("CONTINUE_OK")
                        })
                    }),
                );
                return Some("done");
            }
            _ => {}
        }
        None
    }
}
pub(super) fn install(
    this: &mut WorkbenchController,
    window: &mut Window,
    cx: &mut Context<WorkbenchController>,
) {
    let Some(config) = CONFIG.get().cloned() else {
        return;
    };
    gpui_kit::profiler::set_trace_enabled(true);
    this.e2e = Some(Probe {
        config,
        phase: 0,
        started: Instant::now(),
        frames: FrameTimingCollector::new(),
        presented: 0,
        checks: BTreeMap::new(),
        events: BTreeMap::new(),
        active: String::new(),
        pending: String::new(),
        optimistic: false,
        ack: false,
        initial: this.preferences.value.borrow().session.clone(),
        canonical: BTreeMap::new(),
        history: Vec::new(),
        old_epoch: 0,
        token: None,
        fault_at: None,
        cancel_at: None,
        timings: BTreeMap::new(),
    });
    this.operations.push(cx.spawn_in(window,async move|weak,cx|{
        loop {
            cx.background_executor().timer(Duration::from_millis(50)).await;
            let stop=weak.update_in(cx,|this,window,cx|{
                let Some(mut p)=this.e2e.take() else {return true};
                let result=p.tick(this,window,cx);
                if let Some(reason)=result {
                    let report=serde_json::json!({"schemaVersion":1,"case":p.config.case,"checks":p.checks,"presentedFrames":p.presented,"timings":p.timings,"phase":p.phase,"failure":if reason=="done"{None}else{Some(reason)},"eventCounts":p.events});
                    let path=p.config.root.join(format!("{}.json",p.config.case));
                    this.operations.push(cx.spawn_in(window,async move|weak,cx|{
                        let _=smol::unblock(move||->std::io::Result<()>{let f=fs::OpenOptions::new().write(true).create_new(true).open(path)?;serde_json::to_writer_pretty(f,&report).map_err(std::io::Error::other)}).await;
                        let _=weak.update_in(cx,|this,_,cx|crate::orderly_quit(&this.runtime,&this.startup,&this.quitting,&this.preferences,cx));
                    }));true
                }else{this.e2e=Some(p);window.refresh();false}
            }).unwrap_or(true);
            if stop {break}
        }
    }));
}
