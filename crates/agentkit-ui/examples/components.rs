//! Run with cargo run -p agentkit-ui --features samples --example components.
use agentkit_ui::samples::ComponentSamples;
use gpui_kit::{
    component::TitleBar,
    profiler::{FrameEvent, FrameTimingCollector},
    *,
};
use std::{
    cell::Cell,
    fs::OpenOptions,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let report = match args.as_slice() {
        [] => None,
        [flag, path] if flag == "--smoke-report" => Some(PathBuf::from(path)),
        _ => return Err("Usage: components [--smoke-report NEW.json]".into()),
    };
    let completed = Rc::new(Cell::new(report.is_none()));
    let result = completed.clone();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(1280.), px(820.)), cx)),
                window_min_size: Some(size(px(960.), px(600.))),
                ..TitleBar::window_options()
            };
            if let Err(error) = gpui_kit::open_window(options, cx, move |window, cx| {
                window.set_window_title("ShipAgent · 组件样板");
                let view = cx.new(|cx| ComponentSamples::new(window, cx));
                if let Some(report) = report {
                    gpui_kit::profiler::set_trace_enabled(true);
                    let frames = FrameTimingCollector::new();
                    window.on_next_frame(move |window, cx| {
                        report_after_present(
                            window,
                            cx,
                            frames,
                            report,
                            completed,
                            Instant::now(),
                            1,
                        );
                    });
                }
                view
            }) {
                eprintln!("Unable to open component sample: {error}");
                cx.quit();
            }
            cx.activate(true);
        });
    if !result.get() {
        return Err("Native sample did not report a rendered frame".into());
    }
    Ok(())
}

// Frame callbacks can precede rendering on native platforms. Wait for evidence
// instead of treating a fixed callback count as a presentation guarantee.
fn report_after_present(
    window: &mut Window,
    cx: &mut App,
    mut frames: FrameTimingCollector,
    report: PathBuf,
    completed: Rc<Cell<bool>>,
    started: Instant,
    callbacks: usize,
) {
    let submitted = frames.collect_unseen().iter().filter(|frame| {
        matches!(frame, FrameEvent::Present(present) if present.window_id == window.window_handle().window_id())
    }).count();
    if submitted == 0 && started.elapsed() < Duration::from_secs(10) {
        window.refresh();
        window.on_next_frame(move |window, cx| {
            report_after_present(
                window,
                cx,
                frames,
                report,
                completed,
                started,
                callbacks + 1,
            );
        });
        return;
    }
    let size = window.viewport_size();
    let evidence = serde_json::json!({
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "renderedFrame": submitted > 0,
        "submittedFrames": submitted,
        "frameCallbacks": callbacks,
        "elapsedMs": started.elapsed().as_secs_f64() * 1000.,
        "width": f32::from(size.width),
        "height": f32::from(size.height),
    });
    let written = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(report)
        .and_then(|file| {
            serde_json::to_writer_pretty(file, &evidence).map_err(std::io::Error::other)
        });
    match written {
        Ok(()) => completed.set(true),
        Err(error) => eprintln!("Native sample report failed: {error}"),
    }
    cx.quit();
}
