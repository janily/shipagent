//! Native phase-zero application composition and asynchronous backend lifecycle.
#![deny(missing_docs)]
mod controller;
use anyhow::{Context as _, Result};
use gpui_kit::component::{Theme, ThemeMode, ThemeRegistry, TitleBar};
use gpui_kit::*;
use serde::Deserialize;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

gpui_kit::actions!(
    workbench,
    [
        /// Quit after shutting down the local backend.
        Quit
    ]
);

/// Branding values are centralized in the reference application's brand.toml.
#[derive(Clone, Deserialize)]
pub struct Brand {
    /// Displayed application title.
    pub app_name: String,
    /// Application identity for packaging.
    pub bundle_id: String,
    /// Independent backend directory under the user's profile.
    pub data_dir_name: String,
    /// Registered URL scheme for future native deep links.
    pub url_scheme: String,
    /// Initial light/dark theme (runtime preference persistence follows later).
    pub default_theme: String,
}
/// Run a native GPUI application. The window opens before any backend IO.
pub fn run(brand_toml: &str) -> Result<()> {
    let brand: Brand = toml::from_str(brand_toml).context("Invalid brand.toml")?;
    let runtime: controller::RuntimeSlot = Rc::new(RefCell::new(None));
    let startup: controller::StartupSlot = Rc::new(RefCell::new(None));
    gpui_kit::application()
        // Last-window auto quit would bypass our awaited startup/shutdown task.
        .with_quit_mode(QuitMode::Explicit)
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            cx.bind_keys([KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-q"
                } else {
                    "ctrl-q"
                },
                Quit,
                None,
            )]);
            let quitting = Rc::new(Cell::new(false));
            let action_runtime = runtime.clone();
            let action_startup = startup.clone();
            let action_quitting = quitting.clone();
            cx.on_action(move |_: &Quit, cx| {
                orderly_quit(&action_runtime, &action_startup, &action_quitting, cx);
            });
            agentkit_ui::set_locale(
                &std::env::var("AGENTKIT_LANGUAGE").unwrap_or_else(|_| "zh-CN".into()),
            );
            // Reference palette lives here; views consume only Kit semantic roles.
            if ThemeRegistry::global_mut(cx)
                .load_themes_from_str(include_str!("../theme.json"))
                .is_ok()
            {
                let themes = ThemeRegistry::global(cx).themes();
                if let (Some(light), Some(dark)) =
                    (themes.get("Workbench Light"), themes.get("Workbench Dark"))
                {
                    let (light, dark) = (light.clone(), dark.clone());
                    Theme::update(cx, |theme| {
                        theme.light_theme = light;
                        theme.dark_theme = dark;
                    });
                }
            }
            Theme::change(
                if brand.default_theme == "dark" {
                    ThemeMode::Dark
                } else {
                    ThemeMode::Light
                },
                None,
                cx,
            );
            let quit_runtime = runtime.clone();
            cx.on_app_quit(move |_| {
                let running = quit_runtime.borrow_mut().take();
                async move {
                    if let Some(running) = running {
                        running.shutdown_for_exit().await;
                    }
                }
            })
            .detach();
            let close_runtime = runtime.clone();
            let close_startup = startup.clone();
            let close_quitting = quitting.clone();
            cx.on_window_closed(move |cx, _| {
                if cx.windows().is_empty() {
                    orderly_quit(&close_runtime, &close_startup, &close_quitting, cx);
                }
            })
            .detach();
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(1280.), px(820.)), cx)),
                window_min_size: Some(size(px(960.), px(600.))),
                ..TitleBar::window_options()
            };
            if let Err(error) = gpui_kit::open_window(options, cx, |window, cx| {
                window.set_window_title(&brand.app_name);
                cx.new(|cx| {
                    controller::WorkbenchController::new(
                        brand, runtime, startup, quitting, window, cx,
                    )
                })
            }) {
                // Window errors do not contain backend credentials or conversation data.
                eprintln!("Unable to open Workbench window: {error}");
                cx.quit();
            }
            cx.activate(true);
        });
    Ok(())
}

fn orderly_quit(
    runtime: &controller::RuntimeSlot,
    startup: &controller::StartupSlot,
    quitting: &Cell<bool>,
    cx: &mut App,
) {
    if quitting.replace(true) {
        return;
    }
    let starting = startup.borrow_mut().take();
    let runtime = runtime.clone();
    cx.spawn(async move |cx| {
        if let Some(starting) = starting {
            starting.await;
        }
        let running = runtime.borrow_mut().take();
        if let Some(running) = running {
            running.shutdown().await;
        }
        cx.update(|cx| cx.quit());
    })
    .detach();
}
