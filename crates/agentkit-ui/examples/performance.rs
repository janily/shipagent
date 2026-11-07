//! Synthetic native UI workloads; no RPC or real conversation is fabricated.
use agentkit_protocol::generated::Session;
use agentkit_store::{core::turns::Activity, entities::SessionStore};
use agentkit_ui::{
    WorkbenchView,
    tools::{FullToolEditor, ToolCard, ToolOutputView},
};
use gpui_kit::{
    component::*,
    prelude::*,
    profiler::{FrameEvent, FrameTiming, FrameTimingCollector},
    *,
};
use serde_json::{Value, json};
use std::{
    cell::Cell,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};
fn session(index: usize, count: usize) -> Result<Session, serde_json::Error> {
    let messages:Vec<_>=(0..count).map(|i|json!({"id":format!("fixture-{index}-{i}"),"role":if i%2==0{"user"}else{"assistant"},"content":format!("fixture_{i}\n\n```rust\nlet value = 42;\n```\n"),"timestamp":i})).collect();
    serde_json::from_value(
        json!({"id":format!("fixture-{index}"),"workspaceId":"fixture","workspaceName":"fixture","messages":messages,"lastMessageAt":0,"isProcessing":false}),
    )
}
fn resident_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/self/status")
            .ok()?
            .lines()
            .find(|l| l.starts_with("VmRSS:"))?
            .split_whitespace()
            .nth(1)?
            .parse::<u64>()
            .ok()?
            .checked_mul(1024)
    }
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("/bin/ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8(output.stdout)
            .ok()?
            .trim()
            .parse::<u64>()
            .ok()?
            .checked_mul(1024)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}
fn p95(values: &[f64]) -> Option<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    (!sorted.is_empty()).then(|| sorted[(sorted.len() * 95).div_ceil(100) - 1])
}
struct Scene {
    view: Entity<WorkbenchView>,
    stores: Vec<Entity<SessionStore>>,
    activity: Option<Activity>,
    preview: Option<Entity<ToolOutputView>>,
    editor: Option<Entity<FullToolEditor>>,
    frames: FrameTimingCollector,
    draw: Option<FrameTiming>,
    phase: u8,
    phase_frames: usize,
    phase_started: Instant,
    first: Option<Instant>,
    last: Option<Instant>,
    work: Vec<f64>,
    unmeasured: usize,
    started: Instant,
    path: PathBuf,
    done: Rc<Cell<bool>>,
    metrics: Value,
    rss_pending: bool,
    idle_done: bool,
    ten_done: bool,
}
impl Scene {
    fn new(
        path: PathBuf,
        done: Rc<Cell<bool>>,
        started: Instant,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let view = cx.new(|cx| WorkbenchView::new(agentkit_ui::t!("ak.app").into(), window, cx));
        let store = cx.new(|_| SessionStore::new(session(0, 0).unwrap_or_else(|_| unreachable!())));
        view.update(cx, |v, cx| {
            v.connected("fixture".into(), Some("fixture".into()), cx);
            v.bind_session(store, window, cx);
        });
        Self {
            view,
            stores: vec![],
            activity: None,
            preview: None,
            editor: None,
            frames: FrameTimingCollector::new(),
            draw: None,
            phase: 0,
            phase_frames: 0,
            phase_started: Instant::now(),
            first: None,
            last: None,
            work: vec![],
            unmeasured: 0,
            started,
            path,
            done,
            metrics: json!({"schemaVersion":1,"status":"running","profile":env!("AGENTKIT_UI_BUILD_PROFILE"),"optLevel":env!("AGENTKIT_UI_BUILD_OPT_LEVEL"),"platform":std::env::consts::OS,"arch":std::env::consts::ARCH,"workload":{"version":1,"memorySessions":10,"messagesPerMemorySession":100,"historyMessages":10000,"toolLines":100000,"scrollSteps":120},"boundary":"synthetic-native-ui;rss-frontend-only;app-entry-excludes-loader"}),
            rss_pending: false,
            idle_done: false,
            ten_done: false,
        }
    }
    fn reset(&mut self, phase: u8) {
        self.phase = phase;
        self.phase_frames = 0;
        self.phase_started = Instant::now();
        self.first = None;
        self.last = None;
        self.work.clear();
        self.unmeasured = 0;
        self.draw = None;
    }
    fn rss(&mut self, ten: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.rss_pending {
            return;
        }
        self.rss_pending = true;
        cx.spawn_in(window, async move |weak, cx| {
            let value = cx
                .background_executor()
                .spawn(async { resident_bytes() })
                .await;
            let _ = weak.update_in(cx, |this, _, cx| {
                this.metrics[if ten {
                    "tenSessionsRssBytes"
                } else {
                    "idleRssBytes"
                }] = json!(value);
                this.rss_pending = false;
                if ten {
                    this.ten_done = true;
                } else {
                    this.idle_done = true;
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for event in self.frames.collect_unseen() {
            match event {
                FrameEvent::Draw(d) if d.window_id == window.window_handle().window_id() => {
                    self.draw = Some(d)
                }
                FrameEvent::Present(p) if p.window_id == window.window_handle().window_id() => {
                    self.phase_frames += 1;
                    if self.metrics["firstSubmissionMs"].is_null() {
                        self.metrics["firstSubmissionMs"] = json!(
                            p.present_end
                                .saturating_duration_since(self.started)
                                .as_secs_f64()
                                * 1000.
                        );
                    }
                    self.first.get_or_insert(p.present_end);
                    self.last = Some(p.present_end);
                    if let Some(d) = self.draw.take() {
                        if let Some(work) = d.work(p.present_start) {
                            self.work.push(work.as_secs_f64() * 1000.);
                        } else {
                            self.unmeasured += 1;
                        }
                    } else {
                        self.unmeasured += 1;
                    }
                    if self.phase_frames == 1 {
                        let key = match self.phase {
                            2 => Some("historyOpenMs"),
                            4 => Some("toolCollapsedMs"),
                            5 => Some("toolPreviewOpenMs"),
                            6 => Some("toolFullOpenMs"),
                            _ => None,
                        };
                        if let Some(key) = key {
                            self.metrics[key] = json!(
                                p.present_end
                                    .saturating_duration_since(self.phase_started)
                                    .as_secs_f64()
                                    * 1000.
                            );
                        }
                    }
                }
                _ => {}
            }
        }
        if self.started.elapsed() > Duration::from_secs(45) {
            self.finish(false, window, cx);
            return;
        }
        match self.phase {
            0 => {
                if self.phase_frames >= 5 && !self.idle_done {
                    self.rss(false, window, cx);
                }
                if self.idle_done {
                    self.reset(1);
                }
            }
            1 => {
                if self.stores.len() < 10 {
                    let i = self.stores.len() + 1;
                    let Ok(data) = session(i, 100) else {
                        self.finish(false, window, cx);
                        return;
                    };
                    let store = cx.new(|_| SessionStore::new(data));
                    self.view
                        .update(cx, |v, cx| v.bind_session(store.clone(), window, cx));
                    self.stores.push(store);
                } else if self.phase_frames >= 20 && !self.ten_done {
                    self.rss(true, window, cx);
                }
                if self.ten_done {
                    self.reset(2);
                    let Ok(data) = session(100, 10000) else {
                        self.finish(false, window, cx);
                        return;
                    };
                    let store = cx.new(|_| SessionStore::new(data));
                    self.view
                        .update(cx, |v, cx| v.bind_session(store.clone(), window, cx));
                    self.stores.push(store);
                }
            }
            2 => {
                if self.phase_frames >= 1 {
                    self.reset(3);
                }
            }
            3 => {
                if self.phase_frames >= 120 {
                    let elapsed = self
                        .first
                        .zip(self.last)
                        .map(|(a, b)| b.saturating_duration_since(a).as_secs_f64());
                    self.metrics["scroll"] = json!({"frames":self.phase_frames,"measuredFrames":self.work.len(),"unmeasuredFrames":self.unmeasured,"workP95Ms":p95(&self.work),"fps":elapsed.filter(|v|*v>0.).map(|v|(self.phase_frames-1)as f64/v)});
                    let text = (0..100000)
                        .map(|i| format!("fixture_{i}\n"))
                        .collect::<String>()
                        .trim_end_matches('\n')
                        .to_owned();
                    let Ok(activity) = serde_json::from_value(
                        json!({"id":"fixture-tool","type":"tool","status":"completed","timestamp":0,"toolName":"Read","content":text}),
                    ) else {
                        self.finish(false, window, cx);
                        return;
                    };
                    self.activity = Some(activity);
                    self.reset(4);
                } else {
                    let count = self.view.read(cx).scroll_state().read(cx).item_count();
                    let index = (self.phase_frames * 83) % count.max(1);
                    self.view
                        .read(cx)
                        .scroll_state()
                        .clone()
                        .update(cx, |s, cx| s.scroll_to_item(index, cx));
                }
            }
            4 => {
                if self.phase_frames >= 5 {
                    self.reset(5);
                    if let Some(activity) = &self.activity {
                        self.preview = Some(cx.new(|cx| ToolOutputView::new(activity, window, cx)));
                        self.metrics["previewLines"] = json!(
                            self.preview.as_ref().map(|p| p
                                .read(cx)
                                .preview_text()
                                .lines()
                                .count())
                        );
                    }
                }
            }
            5 => {
                if self.phase_frames >= 5 {
                    self.reset(6);
                    let text = self
                        .activity
                        .as_ref()
                        .and_then(|a| a.content.clone())
                        .unwrap_or_default();
                    self.editor =
                        Some(cx.new(|cx| FullToolEditor::new(text, "plaintext", window, cx)));
                    self.metrics["fullEditorLines"] = json!(
                        self.editor.as_ref().map(|e| e
                            .read(cx)
                            .state()
                            .read(cx)
                            .value()
                            .lines()
                            .count())
                    );
                }
            }
            6 => {
                if self.phase_frames >= 5 {
                    self.finish(true, window, cx);
                    return;
                }
            }
            _ => return,
        }
        cx.notify();
        // This callback runs outside rendering; animation requests require a
        // current rendered entity in this GPUI version. Refresh the window.
        window.refresh();
        let weak = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            let _ = weak.update(cx, |this, cx| this.tick(window, cx));
        });
    }
    fn finish(&mut self, passed: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.phase = 7;
        self.metrics["status"] = json!(if passed { "measured" } else { "invalid" });
        let path = self.path.clone();
        let metrics = self.metrics.clone();
        cx.spawn_in(window, async move |weak, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let file = std::fs::OpenOptions::new()
                        .create_new(true)
                        .write(true)
                        .open(path)?;
                    serde_json::to_writer_pretty(file, &metrics).map_err(std::io::Error::other)
                })
                .await;
            let _ = weak.update_in(cx, |this, _, cx| {
                this.done.set(result.is_ok() && passed);
                cx.quit();
            });
        })
        .detach();
    }
}
impl Render for Scene {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = if self.phase >= 6 {
            self.editor.clone().map(IntoElement::into_any_element)
        } else if self.phase == 5 {
            self.preview.clone().map(IntoElement::into_any_element)
        } else if self.phase == 4 {
            self.activity
                .as_ref()
                .map(|a| ToolCard::new(a, false).into_any_element())
        } else {
            Some(self.view.clone().into_any_element())
        };
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .children(body)
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let path = match args.as_slice() {
        [flag, path] if flag == "--report" => PathBuf::from(path),
        _ => return Err("performance --report NEW.json".into()),
    };
    let done = Rc::new(Cell::new(false));
    let result = done.clone();
    let started = Instant::now();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Light, None, cx);
            agentkit_ui::set_locale("zh-CN");
            gpui_kit::profiler::set_trace_enabled(true);
            let opened = gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(1280.), px(820.)), cx)),
                    ..WindowOptions::default()
                },
                cx,
                |window, cx| {
                    let scene = cx.new(|cx| Scene::new(path, done, started, window, cx));
                    let weak = scene.downgrade();
                    window.on_next_frame(move |window, cx| {
                        let _ = weak.update(cx, |s, cx| s.tick(window, cx));
                    });
                    scene
                },
            );
            if opened.is_err() {
                cx.quit();
            }
            cx.activate(true);
        });
    if !result.get() {
        std::process::exit(2);
    }
    Ok(())
}
