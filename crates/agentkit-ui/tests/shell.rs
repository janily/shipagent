#![cfg(feature = "samples")]
#![allow(clippy::unwrap_used)]
use agentkit_ui::{WorkbenchIntent, WorkbenchView, layout::ShellPreferences};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, Point, TestAppContext, WindowBounds,
    WindowOptions, component::WindowExt, point, px, size, test::TestWindowExt,
};
use std::{cell::RefCell, rc::Rc};
fn open(
    cx: &mut TestAppContext,
    width: f32,
    connected: bool,
) -> (AnyWindowHandle, Entity<WorkbenchView>) {
    open_sized(cx, width, 820., connected)
}
fn open_sized(
    cx: &mut TestAppContext,
    width: f32,
    height: f32,
    connected: bool,
) -> (AnyWindowHandle, Entity<WorkbenchView>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        let (w, v) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(width), px(height)),
                })),
                ..WindowOptions::default()
            },
            cx,
            |window, cx| cx.new(|cx| WorkbenchView::new("Workbench".into(), window, cx)),
        )
        .unwrap();
        if connected {
            w.update(cx, |_, _, cx| {
                v.update(cx, |v, cx| {
                    v.connected("Workspace".into(), Some("fixture-model".into()), cx)
                })
            })
            .unwrap();
        }
        (w, v)
    })
}
#[gpui_kit::test]
fn startup_skeleton_is_visible_before_backend_data(cx: &mut TestAppContext) {
    let (w, _) = open(cx, 1280., false);
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("startup-skeleton").is_some());
        assert_eq!(
            window.find("connection-status").label(),
            Some(agentkit_ui::tr("ak.starting").as_str())
        );
    })
    .unwrap();
}
#[gpui_kit::test]
fn wide_context_and_focus_mode_preserve_expansion_preference(cx: &mut TestAppContext) {
    let (w, v) = open(cx, 1440., true);
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        assert!(window.try_find("right-context").is_none());
        assert!(window.try_find("new-session").is_some());
        window.click("toggle-right-panel", cx);
        window.render_frame(cx);
        window.render_frame(cx);
        assert!(window.try_find("right-context").is_some());
        assert!(!window.has_active_sheet(cx));
        window.click("toggle-focus", cx);
        window.render_frame(cx);
        assert!(window.try_find("right-context").is_none());
        assert!(window.try_find("new-session").is_none());
        window.click("toggle-focus", cx);
        window.render_frame(cx);
        assert!(window.try_find("right-context").is_some());
        assert!(window.try_find("new-session").is_some());
        let _ = v;
    })
    .unwrap();
}
#[gpui_kit::test]
fn narrow_context_and_sidebar_use_dismissible_sheets(cx: &mut TestAppContext) {
    let (w, _) = open(cx, 1000., true);
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        assert!(window.try_find("new-session").is_none());
        window.click("toggle-right-panel", cx);
        window.render_frame(cx);
        assert!(window.has_active_sheet(cx));
        assert!(window.try_find("right-context").is_some());
        window.click("panel-plan", cx);
        window.render_frame(cx);
        assert!(window.try_find("panel-tasks").is_some());
        window.press("escape", cx);
        window.render_frame(cx);
        assert!(!window.has_active_sheet(cx));
        window.click("toggle-sidebar", cx);
        window.render_frame(cx);
        assert!(window.has_active_sheet(cx));
        assert!(window.try_find("new-session").is_some());
        window.press("escape", cx);
        window.render_frame(cx);
        assert!(!window.has_active_sheet(cx));
    })
    .unwrap();
}
#[gpui_kit::test]
fn drag_emits_clamped_user_widths(cx: &mut TestAppContext) {
    let (w, v) = open(cx, 1440., true);
    let saved = Rc::new(RefCell::new(Vec::new()));
    cx.update(|cx| {
        let saved = saved.clone();
        cx.subscribe(&v, move |_, e, cx| {
            if let WorkbenchIntent::LayoutChanged(p) = e {
                saved.borrow_mut().push(p.clone());
            }
            let _ = cx;
        })
        .detach();
    });
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        let handles = window.find_all(("resizable-handle", 0usize));
        let from = handles.first().unwrap().bounds().center();
        window.drag(from, from + point(px(80.), px(0.)), cx);
        window.render_frame(cx);
    })
    .unwrap();
    let states = saved.borrow();
    assert!(!states.is_empty());
    assert!(states.last().unwrap().sidebar_width > 280.);
    assert!(states.last().unwrap().sidebar_width <= 420.);
}
#[gpui_kit::test]
fn restored_layout_and_responsive_collapse_do_not_emit_saves(cx: &mut TestAppContext) {
    let (w, v) = open(cx, 1000., true);
    let saved = Rc::new(RefCell::new(0));
    cx.update(|cx| {
        let saved = saved.clone();
        cx.subscribe(&v, move |_, e, _| {
            if matches!(e, WorkbenchIntent::LayoutChanged(_)) {
                *saved.borrow_mut() += 1;
            }
        })
        .detach();
    });
    cx.update_window(w, |_, window, cx| {
        v.update(cx, |v, cx| {
            v.apply_shell(
                ShellPreferences {
                    sidebar_width: 320.,
                    right_open: true,
                    ..Default::default()
                },
                true,
                window,
                cx,
            )
        });
        window.render_frame(cx);
        assert!(window.has_active_sheet(cx));
        v.update(cx, |v, cx| v.viewport_changed(window, cx));
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(*saved.borrow(), 0);
}
#[gpui_kit::test]
fn restored_column_widths_remain_exact_after_layout_settles(cx: &mut TestAppContext) {
    let (w, v) = open(cx, 1440., true);
    cx.update_window(w, |_, window, cx| {
        v.update(cx, |v, cx| {
            v.apply_shell(
                ShellPreferences {
                    sidebar_width: 320.,
                    right_width: 360.,
                    right_open: true,
                    ..Default::default()
                },
                true,
                window,
                cx,
            )
        });
        for _ in 0..4 {
            window.render_frame(cx);
        }
        let width = f32::from(window.find("session-sidebar").bounds().size.width);
        assert!((width - 320.).abs() < 2., "sidebar width {width}");
        assert!((f32::from(window.find("right-context").bounds().size.width) - 360.).abs() < 2.);
    })
    .unwrap();
}
#[gpui_kit::test]
fn failed_preference_writer_status_is_visible(cx: &mut TestAppContext) {
    let (w, v) = open(cx, 1280., true);
    cx.update_window(w, |_, window, cx| {
        v.update(cx, |v, cx| v.set_preferences_writable(false, cx));
        window.render_frame(cx);
        assert_eq!(
            window.find("preferences-status").label(),
            Some(agentkit_ui::tr("ak.prefs_readonly").as_str())
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn status_bar_uses_app_background_count_even_without_active_session(cx: &mut TestAppContext) {
    let (w, v) = open(cx, 1280., true);
    cx.update_window(w, |_, window, cx| {
        let app = cx.new(|_| agentkit_store::entities::AppState {
            background_tasks: 2,
            ..Default::default()
        });
        let workspace = cx.new(|_| agentkit_store::entities::WorkspaceStore::default());
        v.update(cx, |v, cx| v.bind_context(app.clone(), workspace, cx));
        window.render_frame(cx);
        assert_eq!(
            window.find("background-task-count").label(),
            Some(format!("{} 2", agentkit_ui::tr("ak.tasks")).as_str())
        );
        app.update(cx, |s, cx| {
            s.background_tasks = 1;
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(
            window.find("background-task-count").label(),
            Some(format!("{} 1", agentkit_ui::tr("ak.tasks")).as_str())
        );
    })
    .unwrap();
}
#[gpui_kit::test]
fn minimum_native_size_keeps_composer_visible_and_uses_context_sheet(cx: &mut TestAppContext) {
    let (w, v) = open_sized(cx, 960., 600., true);
    cx.update_window(w, |_, window, cx| {
        v.update(cx, |v, cx| v.viewport_changed(window, cx));
        window.render_frame(cx);
        window.render_frame(cx);
        assert!(window.try_find("session-sidebar").is_none());
        assert!(window.find("send").bounds().bottom() <= px(600.));
        window.click("toggle-right-panel", cx);
        window.render_frame(cx);
        assert!(window.has_active_sheet(cx));
        assert!(window.find("right-context").bounds().size.width <= px(560.));
    })
    .unwrap();
}
