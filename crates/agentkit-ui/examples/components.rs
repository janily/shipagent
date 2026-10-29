//! Run with cargo run -p agentkit-ui --features samples --example components.
use agentkit_ui::samples::ComponentSamples;
use gpui_kit::{component::TitleBar, *};
use std::{cell::Cell, fs::OpenOptions, path::PathBuf, rc::Rc};

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
                    window.on_next_frame(move |window, cx| {
                        let size = window.viewport_size();
                        let evidence = serde_json::json!({
                            "platform": std::env::consts::OS,
                            "arch": std::env::consts::ARCH,
                            "renderedFrame": true,
                            "width": f32::from(size.width),
                            "height": f32::from(size.height),
                        });
                        let written = OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(report)
                            .and_then(|file| {
                                serde_json::to_writer_pretty(file, &evidence)
                                    .map_err(std::io::Error::other)
                            });
                        match written {
                            Ok(()) => completed.set(true),
                            Err(error) => eprintln!("Native sample report failed: {error}"),
                        }
                        cx.quit();
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
