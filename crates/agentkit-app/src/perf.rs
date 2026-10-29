//! Optional benchmark instrumentation; absent from default builds.
use super::*;
use gpui_kit::profiler::{FrameEvent, FrameTiming, FrameTimingCollector};
use serde::Serialize;
use std::{collections::BTreeMap, fs, io::Write, time::Instant};

#[derive(Clone)]
pub(super) struct Config {
    root: PathBuf,
    model_url: String,
}
impl Config {
    pub(super) fn from_env() -> Result<Option<Self>> {
        let Some(root) = std::env::var_os("AGENTKIT_PERF_ROOT") else {
            return Ok(None);
        };
        let root = PathBuf::from(root).canonicalize()?;
        anyhow::ensure!(
            std::env::var_os("AGENTKIT_REMOTE_URL").is_none()
                && std::env::var_os("AGENTKIT_REMOTE_TOKEN").is_none(),
            "Perf cannot attach to a remote backend"
        );
        anyhow::ensure!(
            fs::read_to_string(root.join(".shipagent-perf"))? == "shipagent-perf-v1",
            "Perf requires an isolated runner directory"
        );
        anyhow::ensure!(
            std::env::var_os("AGENTKIT_CONFIG_DIR").map(PathBuf::from) == Some(root.join("data")),
            "Perf requires isolated CONFIG_DIR"
        );
        anyhow::ensure!(
            std::env::var_os("AGENTKIT_WORKING_DIR").map(PathBuf::from)
                == Some(root.join("fixture")),
            "Perf requires isolated working directory"
        );
        anyhow::ensure!(
            !root.join("data").exists() || fs::read_dir(root.join("data"))?.next().is_none(),
            "Perf config must be empty"
        );
        anyhow::ensure!(
            !root.join("native.json").exists(),
            "Perf report already exists"
        );
        let model_url = std::env::var("AGENTKIT_PERF_MODEL_URL")?;
        // The runner owns this loopback model; never submit benchmark data externally.
        let prefix = "http://127.0.0.1:";
        let port = model_url
            .strip_prefix(prefix)
            .and_then(|s| s.strip_suffix("/v1"))
            .and_then(|s| s.parse::<u16>().ok());
        anyhow::ensure!(
            port.is_some_and(|p| p > 0),
            "Perf model must be a loopback HTTP endpoint"
        );
        Ok(Some(Self { root, model_url }))
    }
}

#[derive(Serialize)]
struct Distribution {
    count: usize,
    p50_ms: Option<f64>,
    p95_ms: Option<f64>,
    max_ms: Option<f64>,
}
fn distribution(samples: &[f64]) -> Distribution {
    let mut values = samples.to_vec();
    values.sort_by(f64::total_cmp);
    let percentile =
        |p: usize| (!values.is_empty()).then(|| values[(values.len() * p).div_ceil(100) - 1]);
    Distribution {
        count: values.len(),
        p50_ms: percentile(50),
        p95_ms: percentile(95),
        max_ms: values.last().copied(),
    }
}
#[derive(Default)]
struct Frames {
    pending: Option<FrameTiming>,
    draw: Vec<f64>,
    work: Vec<f64>,
    dirty: Vec<f64>,
    intervals: Vec<f64>,
    submission: Vec<f64>,
    unmeasured: u64,
    unpresented: u64,
    unpaired: u64,
    last_present: Option<Instant>,
}
impl Frames {
    fn observe(&mut self, event: FrameEvent, window_id: WindowId) {
        match event {
            FrameEvent::Draw(frame) if frame.window_id == window_id => {
                self.draw.push(frame.draw_duration().as_secs_f64() * 1000.);
                if self.pending.replace(frame).is_some() {
                    self.unpresented += 1;
                }
            }
            FrameEvent::Present(present) if present.window_id == window_id => {
                self.last_present = Some(present.present_end);
                self.submission
                    .push(present.present_duration().as_secs_f64() * 1000.);
                if let Some(interval) = present.animation_interval {
                    self.intervals.push(interval.as_secs_f64() * 1000.);
                }
                if let Some(frame) = self.pending.take() {
                    if let Some(work) = frame.work(present.present_start) {
                        self.work.push(work.as_secs_f64() * 1000.);
                    } else {
                        self.unmeasured += 1;
                    }
                    if let Some(dirty) = frame.dirty_at {
                        self.dirty.push(
                            present
                                .present_end
                                .saturating_duration_since(dirty)
                                .as_secs_f64()
                                * 1000.,
                        );
                    }
                } else {
                    self.unpaired += 1;
                }
            }
            _ => {}
        }
    }
}

pub(super) struct Capture {
    config: Config,
    window_id: WindowId,
    session: Option<String>,
    collector: Option<FrameTimingCollector>,
    started: Option<Instant>,
    terminal: Option<Instant>,
    outcome: Option<&'static str>,
    failed: bool,
    counts: BTreeMap<String, u64>,
    delta_bytes: usize,
    frames: Frames,
    previous_trace: bool,
    updates: Vec<(Instant, Instant)>,
    update_work: Vec<f64>,
    active_updates: usize,
}
impl Capture {
    pub(super) fn new(config: Config, window_id: WindowId) -> Self {
        Self {
            config,
            window_id,
            session: None,
            collector: None,
            started: None,
            terminal: None,
            outcome: None,
            failed: false,
            counts: BTreeMap::new(),
            delta_bytes: 0,
            frames: Frames::default(),
            previous_trace: false,
            updates: vec![],
            update_work: vec![],
            active_updates: 0,
        }
    }
    pub(super) fn observe(&mut self, event: &SessionEvent) {
        if self.session.as_deref() != Some(&event.session_id) || self.terminal.is_some() {
            return;
        }
        *self.counts.entry(event.kind.clone()).or_default() += 1;
        if event.kind == "text_delta" {
            if self.started.is_none() {
                self.previous_trace = gpui_kit::profiler::trace_enabled();
                gpui_kit::profiler::set_trace_enabled(true);
                self.collector = Some(FrameTimingCollector::new());
                self.started = Some(Instant::now());
            }
            self.delta_bytes += event.payload["delta"].as_str().map_or(0, str::len);
        }
        if matches!(event.kind.as_str(), "error" | "typed_error")
            || (event.kind == "tool_result" && event.payload["isError"] == true)
        {
            self.failed = true;
        }
        if matches!(event.kind.as_str(), "complete" | "interrupted") {
            self.terminal = Some(Instant::now());
            self.outcome = Some(if self.failed {
                "failed"
            } else if event.kind == "complete" {
                "complete"
            } else {
                "interrupted"
            });
        }
    }
    pub(super) fn note_update(&mut self, received: Instant, started: Instant, active: bool) {
        if self.started.is_some() {
            self.active_updates += usize::from(active);
            let applied = Instant::now();
            self.updates.push((received, applied));
            self.update_work
                .push(applied.saturating_duration_since(started).as_secs_f64() * 1000.);
        }
    }
    fn report(mut self, final_bytes: usize) -> serde_json::Value {
        let mut update_latencies = vec![];
        let mut next_update = 0;
        let mut pending_draw = None;
        if let Some(collector) = &mut self.collector {
            for event in collector.collect_unseen() {
                match event {
                    FrameEvent::Draw(frame) if frame.window_id == self.window_id => {
                        pending_draw = Some(frame.draw_start);
                    }
                    FrameEvent::Present(present) if present.window_id == self.window_id => {
                        if let Some(draw_started) = pending_draw.take() {
                            while let Some((received, applied)) = self.updates.get(next_update) {
                                if *applied > draw_started {
                                    break;
                                }
                                update_latencies.push(
                                    present
                                        .present_end
                                        .saturating_duration_since(*received)
                                        .as_secs_f64()
                                        * 1000.,
                                );
                                next_update += 1;
                            }
                        }
                    }
                    _ => {}
                }
                self.frames.observe(event, self.window_id);
            }
            gpui_kit::profiler::set_trace_enabled(self.previous_trace);
        }
        let final_frame = self
            .terminal
            .zip(self.frames.last_present)
            .is_some_and(|(terminal, present)| present >= terminal);
        serde_json::json!({
            "schemaVersion":1,"profile":env!("AGENTKIT_BUILD_PROFILE"),"optLevel":env!("AGENTKIT_BUILD_OPT_LEVEL"),
            "outcome":self.outcome.unwrap_or("incomplete"),"events":self.counts,"deltaBytes":self.delta_bytes,
            "firstDeltaToTerminalMs":self.started.zip(self.terminal).map(|(a,b)|b.saturating_duration_since(a).as_secs_f64()*1000.),
            "activeUpdateFraction":if self.updates.is_empty(){None}else{Some(self.active_updates as f64/self.updates.len() as f64)},
            "finalAssistantBytes":final_bytes,"completionTailMs":100,"finalFramePresented":final_frame,
            "frameWork":distribution(&self.frames.work),"draw":distribution(&self.frames.draw),
            "controllerUpdate":distribution(&self.update_work),"batchReceiptToPresent":distribution(&update_latencies),"unpresentedUpdates":self.updates.len()-next_update,
            "dirtyToPresent":distribution(&self.frames.dirty),"submission":distribution(&self.frames.submission),"animationInterval":distribution(&self.frames.intervals),
            "unmeasuredFrames":self.frames.unmeasured,"unpresentedFrames":self.frames.unpresented+u64::from(self.frames.pending.is_some()),"unpairedPresentFrames":self.frames.unpaired
        })
    }
}

pub(super) fn launch(
    this: &mut WorkbenchController,
    window: &mut Window,
    cx: &mut Context<WorkbenchController>,
) {
    let Some(config) = this.perf.as_ref().map(|p| p.config.clone()) else {
        return;
    };
    let Some(api) = this.api() else { return };
    this.operations
        .push(cx.spawn_in(window, async move |weak, cx| {
            let setup = api
                .setup_endpoint(&config.model_url, "fixture-model", None)
                .await;
            let _ = weak.update_in(cx, |this, window, cx| {
                if let Err(_error) = setup {
                    fail(this, "setup_failed", window, cx);
                    return;
                }
                if let Some(capture) = &mut this.perf {
                    capture.session = this.state.as_ref().map(|s| s.session.id.clone());
                }
                this.view.update(cx, |view, cx| {
                    view.connected("Benchmark".into(), Some("fixture-model".into()), cx)
                });
                this.handle(
                    WorkbenchIntent::Submit("Read proof.txt and report its marker".into()),
                    window,
                    cx,
                );
            });
        }));
}
pub(super) fn watchdog(
    this: &mut WorkbenchController,
    window: &mut Window,
    cx: &mut Context<WorkbenchController>,
) {
    if this.perf.is_none() {
        return;
    }
    this.operations
        .push(cx.spawn_in(window, async move |weak, cx| {
            cx.background_executor()
                .timer(Duration::from_secs(45))
                .await;
            let _ = weak.update_in(cx, |this, window, cx| fail(this, "timeout", window, cx));
        }));
}
fn fail(
    this: &mut WorkbenchController,
    outcome: &'static str,
    window: &mut Window,
    cx: &mut Context<WorkbenchController>,
) {
    if let Some(capture) = &mut this.perf {
        capture.outcome = Some(outcome);
        capture.terminal = Some(Instant::now());
    }
    finish_if_terminal(this, window, cx);
}
pub(super) fn finish_if_terminal(
    this: &mut WorkbenchController,
    window: &mut Window,
    cx: &mut Context<WorkbenchController>,
) {
    if !this.perf.as_ref().is_some_and(|p| p.terminal.is_some()) {
        return;
    }
    let Some(capture) = this.perf.take() else {
        return;
    };
    let path = capture.config.root.join("native.json");
    let final_bytes = this
        .state
        .as_ref()
        .and_then(|s| s.session.messages.iter().rfind(|m| m.role == "assistant"))
        .map_or(0, |m| m.content.len());
    this.operations
        .push(cx.spawn_in(window, async move |weak, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let report = capture.report(final_bytes);
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut file = fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(path)?;
                    file.write_all(serde_json::to_string_pretty(&report)?.as_bytes())?;
                    Ok::<_, anyhow::Error>(())
                })
                .await;
            if result.is_err() {
                eprintln!("Benchmark report could not be written");
            }
            let _ = weak.update_in(cx, |this, _, cx| {
                crate::orderly_quit(&this.runtime, &this.startup, &this.quitting, cx)
            });
        }));
}

#[cfg(test)]
mod tests {
    use super::{Duration, FrameEvent, FrameTiming, Frames, Instant, WindowId, distribution};
    #[::core::prelude::v1::test]
    fn empty_samples_are_unmeasured_and_nearest_rank_p95_is_not_an_average() {
        assert!(distribution(&[]).p95_ms.is_none());
        assert_eq!(
            distribution(&(1..=20).map(f64::from).collect::<Vec<_>>()).p95_ms,
            Some(19.)
        );
    }
    #[::core::prelude::v1::test]
    fn work_excludes_display_interval_and_unmeasured_frames_are_not_zero() {
        let mut frames = Frames::default();
        let start = Instant::now();
        let id = WindowId::from(1);
        frames.observe(
            FrameEvent::Draw(FrameTiming {
                window_id: id,
                dirty_at: Some(start),
                invalidations: 1,
                draw_start: start + Duration::from_millis(4),
                draw_end: start + Duration::from_millis(6),
                refresh_interval: None,
                signal_at: Some(start + Duration::from_millis(1)),
                phases: Default::default(),
            }),
            id,
        );
        frames.observe(
            FrameEvent::Present(gpui_kit::profiler::PresentTiming {
                window_id: id,
                present_start: start + Duration::from_millis(8),
                present_end: start + Duration::from_millis(10),
                animation_interval: Some(Duration::from_millis(50)),
            }),
            id,
        );
        assert_eq!(frames.work, [7.]);
        assert_eq!(frames.draw, [2.]);
        assert_eq!(frames.dirty, [10.]);
        assert_eq!(frames.intervals, [50.]);
        frames.observe(
            FrameEvent::Draw(FrameTiming {
                window_id: id,
                dirty_at: None,
                invalidations: 1,
                draw_start: start,
                draw_end: start,
                refresh_interval: None,
                signal_at: None,
                phases: Default::default(),
            }),
            id,
        );
        frames.observe(
            FrameEvent::Present(gpui_kit::profiler::PresentTiming {
                window_id: id,
                present_start: start,
                present_end: start,
                animation_interval: None,
            }),
            id,
        );
        assert_eq!(frames.unmeasured, 1);
        assert_eq!(frames.work.len(), 1);
    }
}
