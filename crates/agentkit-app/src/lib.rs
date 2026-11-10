//! Native application shell, local preferences and asynchronous backend lifecycle.
#![deny(missing_docs)]
/// Central native application shell actions.
pub mod actions;
mod controller;
mod draft_io;
mod draft_writer;
mod exit;
mod history;
mod keymap;
mod menus;
#[cfg(test)]
mod test_persistence;
use actions::Quit;
use preferences::{Appearance, Geometry, PreferenceStore, Preferences};
/// Profile-scoped frontend single-instance negotiation.
pub mod instance;
/// Client-owned, validated preference document.
pub mod preferences;
use anyhow::{Context as _, Result};
use gpui_kit::component::{Theme, ThemeMode, ThemeRegistry, TitleBar, WindowExt};
use gpui_kit::*;
use serde::Deserialize;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

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
    /// Initial appearance when no client preferences exist (light/dark/system).
    pub default_theme: String,
}
/// Run a native GPUI application. The window opens before any backend IO.
pub fn run(brand_toml: &str) -> Result<()> {
    #[cfg(feature = "e2e")]
    controller::e2e::configure()?;
    #[cfg(feature = "perf")]
    controller::perf::start_process();
    let brand: Brand = toml::from_str(brand_toml).context("Invalid brand.toml")?;
    let directory = client_data_dir(&brand)?;
    let instance = match instance::claim(&directory, &brand.bundle_id)? {
        instance::Claim::Forwarded => return Ok(()),
        instance::Claim::Primary(instance) => instance,
    };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("AGENTKIT_LOG")
                .unwrap_or_else(|_| "warn".into()),
        )
        .with_writer(std::io::stderr)
        .try_init();
    #[cfg(feature = "perf")]
    controller::validate_perf()?;
    let mut defaults = Preferences {
        appearance: match brand.default_theme.as_str() {
            "dark" => Appearance::Dark,
            "system" => Appearance::System,
            _ => Appearance::Light,
        },
        ..Default::default()
    };
    if let Ok(language) = std::env::var("AGENTKIT_LANGUAGE") {
        defaults.language = language;
        defaults.validate();
    }
    let loaded = preferences::load(&directory.join("prefs.json"), defaults.clone());
    let prefs = match loaded {
        Ok(mut prefs) => {
            if let Ok(language) = std::env::var("AGENTKIT_LANGUAGE") {
                prefs.language = language;
                prefs.validate();
            }
            PreferenceStore::new(directory.join("prefs.json"), prefs)
        }
        Err(_) => {
            eprintln!("Preference file unavailable; preserving it and using defaults");
            PreferenceStore::readonly(defaults)
        }
    };
    let keymap_path = directory.join("keymap.json");
    let keymap_source = keymap::read(&keymap_path);
    let focus = instance.focus.clone();
    let _instance = instance;
    let runtime: controller::RuntimeSlot = Rc::new(RefCell::new(None));
    let startup: controller::StartupSlot = Rc::new(RefCell::new(None));
    gpui_kit::application()
        // Last-window auto quit would bypass our awaited startup/shutdown task.
        .with_quit_mode(QuitMode::Explicit)
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            let capabilities = agentkit_capabilities::client_caps::NativeCapabilities::install(cx);
            cx.set_global(exit::ExitState::default());
            let quitting = Rc::new(Cell::new(false));
            let action_runtime = runtime.clone();
            let action_startup = startup.clone();
            let action_quitting = quitting.clone();
            let action_prefs = prefs.clone();
            cx.on_action(move |_: &Quit, cx| {
                if !exit::allow(cx) {
                    return;
                }
                orderly_quit(
                    &action_runtime,
                    &action_startup,
                    &action_quitting,
                    &action_prefs,
                    false,
                    cx,
                );
            });
            let force_runtime = runtime.clone();
            let force_startup = startup.clone();
            let force_quitting = quitting.clone();
            let force_prefs = prefs.clone();
            cx.on_action(move |_: &actions::ConfirmedQuit, cx| {
                orderly_quit(
                    &force_runtime,
                    &force_startup,
                    &force_quitting,
                    &force_prefs,
                    false,
                    cx,
                )
            });
            let discard_runtime = runtime.clone();
            let discard_startup = startup.clone();
            let discard_quitting = quitting.clone();
            let discard_prefs = prefs.clone();
            cx.on_action(move |_: &actions::DiscardDraftsAndQuit, cx| {
                orderly_quit(
                    &discard_runtime,
                    &discard_startup,
                    &discard_quitting,
                    &discard_prefs,
                    true,
                    cx,
                )
            });
            cx.on_action(|_: &actions::CloseWindow, cx| cx.dispatch_action(&Quit));
            cx.on_action(|_: &actions::Hide, cx| cx.hide());
            cx.on_action(|_: &actions::About, cx| {
                if let Some(w) = cx.active_window() {
                    let _ = w.update(cx, |_, w, cx| {
                        use gpui_kit::component::WindowExt;
                        if !w.has_active_dialog(cx) {
                            w.open_alert_dialog(cx, |a, _, _| {
                                a.title(agentkit_ui::t!("ak.menu.about")).description(
                                    agentkit_ui::t!(
                                        "ak.menu.version_info",
                                        version = env!("CARGO_PKG_VERSION"),
                                        notice = include_str!("../../../NOTICE")
                                    ),
                                )
                            });
                        }
                    });
                }
            });
            agentkit_ui::set_locale(&prefs.value.borrow().language);
            let _menu = menus::init(brand.app_name.clone(), cx);
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
            match prefs.value.borrow().appearance {
                Appearance::Light => Theme::change(ThemeMode::Light, None, cx),
                Appearance::Dark => Theme::change(ThemeMode::Dark, None, cx),
                Appearance::System => Theme::sync_system_appearance(None, cx),
            };
            Theme::update(cx, |theme| theme.sheet.margin_top = px(40.));
            let quit_runtime = runtime.clone();
            let quit_prefs = prefs.clone();
            cx.on_app_quit(move |cx| {
                controller::prepare_draft_exit(cx);
                let drafts = cx.try_global::<draft_writer::Writer>().cloned();
                let running = quit_runtime.borrow_mut().take();
                let prefs = quit_prefs.clone();
                async move {
                    exit_tasks(
                        async move {
                            if let Some(drafts) = drafts {
                                let _ = smol::future::race(drafts.flush(), async {
                                    smol::Timer::after(std::time::Duration::from_millis(80)).await;
                                    Err("ak.draft.unsaved".into())
                                })
                                .await;
                            }
                            if let Some(running) = running {
                                running.shutdown_for_exit().await;
                            }
                        },
                        async move {
                            let _ = prefs.finish().await;
                        },
                    )
                    .await;
                }
            })
            .detach();
            let close_runtime = runtime.clone();
            let close_startup = startup.clone();
            let close_quitting = quitting.clone();
            let close_prefs = prefs.clone();
            cx.on_window_closed(move |cx, _| {
                if cx.windows().is_empty() {
                    orderly_quit(
                        &close_runtime,
                        &close_startup,
                        &close_quitting,
                        &close_prefs,
                        false,
                        cx,
                    );
                }
            })
            .detach();
            let options = WindowOptions {
                window_bounds: Some(restored_bounds(prefs.value.borrow().geometry.as_ref(), cx)),
                window_min_size: Some(size(px(960.), px(600.))),
                ..TitleBar::window_options()
            };
            match gpui_kit::open_window(options, cx, |window, cx| {
                window.set_window_title(&brand.app_name);
                cx.new(|cx| {
                    controller::WorkbenchController::new(
                        brand,
                        runtime,
                        startup,
                        quitting,
                        controller::WindowServices {
                            preferences: prefs.clone(),
                            capabilities: capabilities.clone(),
                        },
                        window,
                        cx,
                    )
                })
            }) {
                Ok((handle, controller)) => {
                    let _ = handle.update(cx, |_, window, cx| {
                        window.on_window_should_close(cx, |window, cx| {
                            window.dispatch_action(Box::new(Quit), cx);
                            false
                        })
                    });
                    keymap::install(keymap_path, keymap_source, cx);
                    let weak = controller.downgrade();
                    cx.on_action(move |_: &actions::OpenSettings, cx| {
                        if let Some(w) = cx.active_window() {
                            let _ = w.update(cx, |_, w, cx| {
                                let _ = weak.update(cx, |this, cx| this.global_command(0, w, cx));
                            });
                        }
                    });
                    let weak = controller.downgrade();
                    cx.on_action(move |_: &actions::KeyboardShortcuts, cx| {
                        if let Some(w) = cx.active_window() {
                            let _ = w.update(cx, |_, w, cx| {
                                let _ = weak.update(cx, |this, cx| this.global_command(1, w, cx));
                            });
                        }
                    });
                    let weak = controller.downgrade();
                    cx.on_action(move |_: &actions::ToggleTheme, cx| {
                        if let Some(w) = cx.active_window() {
                            let _ = w.update(cx, |_, w, cx| {
                                let _ = weak.update(cx, |this, cx| this.global_command(2, w, cx));
                            });
                        }
                    });
                    let stops = keymap::escape_stops(cx);
                    controller.update(cx, |c, cx| c.configure_shortcuts(stops, cx));
                    cx.spawn(async move |cx| {
                        while focus.recv().await.is_ok() {
                            let _ = handle.update(cx, |_, window, cx| {
                                window.activate_window();
                                cx.activate(true);
                            });
                        }
                    })
                    .detach();
                }
                Err(error) => {
                    // Window errors do not contain backend credentials or conversation data.
                    eprintln!("Unable to open Workbench window: {error}");
                    cx.quit();
                }
            }
            cx.activate(true);
        });
    Ok(())
}

#[cfg(any(test, feature = "e2e", feature = "perf"))]
fn defer_confirmed_quit(cx: &mut App) {
    // No window lease may be held when App routes an action to its active window.
    cx.defer(|cx| cx.dispatch_action(&actions::ConfirmedQuit));
}

fn orderly_quit(
    runtime: &controller::RuntimeSlot,
    startup: &controller::StartupSlot,
    quitting: &Rc<Cell<bool>>,
    preferences: &PreferenceStore,
    discard_unsaved: bool,
    cx: &mut App,
) {
    if discard_unsaved {
        if let Some(writer) = cx.try_global::<draft_writer::Writer>() {
            writer.unbind();
        }
    } else {
        controller::prepare_draft_exit(cx);
    }
    if quitting.replace(true) {
        return;
    }
    controller::draft_closing(true, cx);
    let drafts = cx.try_global::<draft_writer::Writer>().cloned();
    let quitting = quitting.clone();
    let starting = startup.borrow_mut().take();
    let runtime = runtime.clone();
    let preferences = preferences.clone();
    cx.spawn(async move |cx| {
        if let Some(starting) = starting {
            starting.await;
        }
        if let Some(drafts) = drafts {
            if discard_unsaved {
                drafts.unbind();
            } else if smol::future::race(
                async {
                    // Imports accepted before the editor was frozen must join the final snapshot.
                    while cx.update(|cx| controller::draft_imports_pending(cx)) {
                        smol::Timer::after(std::time::Duration::from_millis(10)).await;
                    }
                    cx.update(controller::prepare_draft_exit);
                    drafts.flush().await
                },
                async {
                    smol::Timer::after(std::time::Duration::from_secs(5)).await;
                    Err("ak.draft.unsaved".into())
                },
            )
            .await
            .is_err()
            {
                quitting.set(false);
                cx.update(|cx| {
                    controller::draft_closing(false, cx);
                    if let Some(window) = cx.active_window() {
                        let _ = window.update(cx, |_, window, cx| {
                            window.open_alert_dialog(cx, |dialog, _, _| {
                                dialog
                                    .title(agentkit_ui::t!("ak.draft.quit_title"))
                                    .description(agentkit_ui::t!("ak.draft.quit_warning"))
                                    .cancel_text(agentkit_ui::t!("ak.menu.keep_working"))
                                    .ok_text(agentkit_ui::t!("ak.draft.quit_discard"))
                                    .on_ok(|_, _, cx| {
                                        cx.dispatch_action(&actions::DiscardDraftsAndQuit);
                                        true
                                    })
                            })
                        });
                    }
                });
                return;
            }
        }
        let running = runtime.borrow_mut().take();
        let _backend_clean = if let Some(running) = running {
            running.shutdown().await
        } else {
            true
        };
        let _ = preferences.finish().await;
        #[cfg(feature = "e2e")]
        controller::e2e::record_shutdown(_backend_clean).await;
        #[cfg(feature = "perf")]
        controller::perf::record_shutdown(_backend_clean).await;
        cx.update(|cx| cx.quit());
    })
    .detach();
}

fn client_data_dir(brand: &Brand) -> Result<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("AGENTKIT_APP_DATA_DIR") {
        return Ok(path.into());
    }
    if let Some(path) = std::env::var_os("AGENTKIT_CONFIG_DIR") {
        return Ok(std::path::PathBuf::from(path).join("client"));
    }
    #[cfg(target_os = "windows")]
    let base: std::path::PathBuf = std::env::var_os("APPDATA")
        .context("Application data directory unavailable")?
        .into();
    #[cfg(target_os = "macos")]
    let base =
        std::path::PathBuf::from(std::env::var_os("HOME").context("User profile unavailable")?)
            .join("Library/Application Support");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = if let Some(path) = std::env::var_os("XDG_CONFIG_HOME") {
        std::path::PathBuf::from(path)
    } else {
        std::path::PathBuf::from(std::env::var_os("HOME").context("User profile unavailable")?)
            .join(".config")
    };
    Ok(base.join(&brand.bundle_id))
}
fn restored_bounds(geometry: Option<&Geometry>, cx: &App) -> WindowBounds {
    let Some(g) = geometry else {
        return WindowBounds::centered(size(px(1280.), px(820.)), cx);
    };
    let visible = cx.displays().iter().any(|display| {
        let b = display.bounds();
        g.title_visible_on(
            f32::from(b.origin.x),
            f32::from(b.origin.y),
            f32::from(b.size.width),
            f32::from(b.size.height),
        )
    });
    let bounds = if visible {
        Bounds {
            origin: point(px(g.x), px(g.y)),
            size: size(px(g.width), px(g.height)),
        }
    } else {
        WindowBounds::centered(size(px(g.width), px(g.height)), cx).get_bounds()
    };
    if g.maximized {
        WindowBounds::Maximized(bounds)
    } else {
        WindowBounds::Windowed(bounds)
    }
}

async fn exit_tasks(
    backend: impl std::future::Future<Output = ()>,
    preferences: impl std::future::Future<Output = ()>,
) {
    smol::future::zip(
        backend,
        smol::future::race(preferences, async {
            smol::Timer::after(std::time::Duration::from_millis(80)).await;
        }),
    )
    .await;
}
#[cfg(test)]
mod exit_policy_tests {
    #![allow(clippy::unwrap_used)]
    #[gpui_kit::test]
    fn deferred_quit_reaches_handler_with_an_active_window(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::test::TestWindowExt;
        use gpui_kit::*;
        use std::cell::Cell;
        use std::rc::Rc;
        struct Empty;
        impl Render for Empty {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div()
            }
        }
        let received = Rc::new(Cell::new(0));
        let window = cx.update(|cx| {
            gpui_kit::init(cx);
            let mark = received.clone();
            cx.on_action(move |_: &super::actions::ConfirmedQuit, _| mark.set(mark.get() + 1));
            gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| cx.new(|_| Empty))
                .unwrap()
                .0
        });
        cx.update_window(window, |_, window, cx| {
            window.activate_window();
            window.render_frame(cx);
            assert!(cx.active_window().is_some());
            super::defer_confirmed_quit(cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert_eq!(received.get(), 1);
    }
    #[::core::prelude::v1::test]
    fn stalled_preferences_never_postpone_starting_backend_cleanup() {
        smol::block_on(async {
            let started = std::rc::Rc::new(std::cell::Cell::new(false));
            let mark = started.clone();
            super::exit_tasks(
                async move {
                    mark.set(true);
                },
                async {
                    assert!(started.get());
                    std::future::pending::<()>().await;
                },
            )
            .await;
            assert!(started.get());
        });
    }
}

mod files;
