#![cfg(feature = "samples")]
#![allow(clippy::unwrap_used)]
use agentkit_protocol::generated as g;
use agentkit_store::entities::AppState;
use agentkit_ui::settings::{
    BackendPreferences, LocalChange, ProxyValues, SettingsIntent, SettingsView,
};
use gpui_kit::{
    AnyWindowHandle, AppContext, Entity, TestAppContext, WindowOptions, test::TestWindowExt,
};
use std::{cell::RefCell, rc::Rc};

fn open(
    cx: &mut TestAppContext,
) -> (
    AnyWindowHandle,
    Entity<SettingsView>,
    Rc<RefCell<Vec<SettingsIntent>>>,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        let app = cx.new(|_| AppState {
            connected: true,
            ..Default::default()
        });
        let (w, view) = gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
            cx.new(SettingsView::new)
        })
        .unwrap();
        let events = Rc::new(RefCell::new(Vec::new()));
        let sink = events.clone();
        cx.subscribe(&view, move |_, e: &SettingsIntent, _| {
            sink.borrow_mut().push(e.clone())
        })
        .detach();
        view.update(cx, |s, cx| s.bind(app, cx));
        w.update(cx, |_, window, cx| {
            view.update(cx, |s, cx| {
                s.backend_preferences(
                    BackendPreferences {
                        proxy: ProxyValues::default(),
                        keep_awake: false,
                        workspace_id: "workspace".into(),
                        permission_mode: g::PermissionMode::Safe,
                        permission_policy: "read-only policy".into(),
                        restart_available: true,
                    },
                    window,
                    cx,
                )
            })
        })
        .unwrap();
        (w, view, events)
    })
}

#[gpui_kit::test]
fn notification_preferences_and_power_use_distinct_local_and_backend_intents(
    cx: &mut TestAppContext,
) {
    let (w, _, events) = open(cx);
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.within("settings-sidebar").click("0-5", cx);
        w.render_frame(cx);
        w.click("settings-notify-complete", cx);
        w.click("settings-notify-approval", cx);
        w.click("settings-keep-awake", cx);
    })
    .unwrap();
    let events = events.borrow();
    assert!(events.iter().any(|e| matches!(
        e,
        SettingsIntent::Local(LocalChange::CompletionNotifications(false))
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        SettingsIntent::Local(LocalChange::ApprovalNotifications(false))
    )));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SettingsIntent::KeepAwake(true)))
    );
}

#[gpui_kit::test]
fn proxy_enter_validates_and_emits_one_proposal_without_a_save_button(cx: &mut TestAppContext) {
    let (w, _, events) = open(cx);
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.within("settings-sidebar").click("0-4", cx);
        w.render_frame(cx);
        w.click("settings-http-proxy", cx);
        w.input("file:///private/fixture-secret", cx);
        w.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("settings-proxy-error").is_some());
        w.click("settings-http-proxy", cx);
        w.press(
            if cfg!(target_os = "macos") {
                "cmd-a"
            } else {
                "ctrl-a"
            },
            cx,
        );
        w.input("http://user:fixture-secret@127.0.0.1:8080", cx);
        w.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(w, |_, w, cx| {
        w.press("enter", cx);
        w.render_frame(cx);
        assert!(w.try_find("settings-proxy-save").is_none());
        assert!(
            !w.within("settings-http-proxy")
                .find("input-state")
                .value()
                .unwrap_or("")
                .contains("fixture-secret")
        );
    })
    .unwrap();
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|e| matches!(e, SettingsIntent::Proxy(_)))
            .count(),
        1
    );
}

#[gpui_kit::test]
fn workspace_permission_choice_is_scoped_and_policy_is_read_only(cx: &mut TestAppContext) {
    let (w, view, events) = open(cx);
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.within("settings-sidebar").click("0-3", cx);
        w.render_frame(cx);
        assert!(w.try_find("settings-permission-policy").is_some());
        w.click(("settings-permission-mode", 2usize), cx);
        view.update(cx, |v, cx| v.status(true, None, None, cx));
        w.render_frame(cx);
        w.click(("settings-permission-mode", 1usize), cx);
    })
    .unwrap();
    assert!(
        matches!(events.borrow().as_slice(), [SettingsIntent::PermissionMode(id, g::PermissionMode::AllowAll)] if id=="workspace")
    );
}

#[gpui_kit::test]
fn editing_inside_proxy_group_and_other_settings_readback_preserve_a_cancelled_draft(
    cx: &mut TestAppContext,
) {
    let (w, view, events) = open(cx);
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.within("settings-sidebar").click("0-4", cx);
        w.render_frame(cx);
        w.click("settings-http-proxy", cx);
        w.input("http://127.0.0.1:8080", cx);
        w.click("settings-https-proxy", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(events.borrow().is_empty());
    cx.update_window(w, |_, w, cx| {
        w.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(events.borrow().len(), 1);
    cx.update_window(w, |_, window, cx| {
        view.update(cx, |s, cx| {
            s.proxy_finished(false, cx);
            s.backend_preferences(
                BackendPreferences {
                    proxy: Default::default(),
                    keep_awake: true,
                    workspace_id: "workspace".into(),
                    permission_mode: g::PermissionMode::Safe,
                    permission_policy: String::new(),
                    restart_available: true,
                },
                window,
                cx,
            );
        })
    })
    .unwrap();
    cx.update_window(w, |_, w, cx| {
        w.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(events.borrow().len(), 2);
    assert!(
        events
            .borrow()
            .iter()
            .all(|e| matches!(e, SettingsIntent::Proxy(_)))
    );
}

#[gpui_kit::test]
fn externally_managed_backend_cannot_change_proxy_or_restart(cx: &mut TestAppContext) {
    let (w, view, events) = open(cx);
    cx.update_window(w, |_, w, cx| {
        view.update(cx, |s, cx| {
            s.backend_preferences(
                BackendPreferences {
                    proxy: Default::default(),
                    keep_awake: false,
                    workspace_id: "remote".into(),
                    permission_mode: g::PermissionMode::Safe,
                    permission_policy: String::new(),
                    restart_available: false,
                },
                w,
                cx,
            )
        });
        w.render_frame(cx);
        w.within("settings-sidebar").click("0-4", cx);
        w.render_frame(cx);
        w.click("settings-proxy-enabled", cx);
        w.within("settings-sidebar").click("0-7", cx);
        w.render_frame(cx);
        w.click("settings-restart-backend", cx);
        w.click("settings-open-logs", cx);
        w.click("settings-export-diagnostics", cx);
    })
    .unwrap();
    assert!(
        !events
            .borrow()
            .iter()
            .any(|e| matches!(e, SettingsIntent::Proxy(_) | SettingsIntent::RestartBackend))
    );
    assert!(
        events
            .borrow()
            .iter()
            .any(|e| matches!(e, SettingsIntent::OpenLogs))
    );
    assert!(
        events
            .borrow()
            .iter()
            .any(|e| matches!(e, SettingsIntent::ExportDiagnostics))
    );
}
