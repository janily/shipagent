#![allow(clippy::unwrap_used)]
use super::*;
use ::core::prelude::v1::test;
#[gpui_kit::test]
fn overrides_remove_defaults_and_reject_invalid_input(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let defaults = resolve(None, cx).unwrap();
        assert!(defaults.iter().any(|e| e.id == "app::NewChat"));
        let entries = resolve(Some(r#"{"version":1,"bindings":[{"action":"app::NewChat","key":"ctrl-k"},{"action":"app::Search","key":null}]}"#), cx).unwrap();
        assert_eq!(entries.iter().find(|e| e.id=="app::NewChat").unwrap().key,"ctrl-k");
        assert!(!entries.iter().any(|e| e.id=="app::Search"));
        for bad in ["{",r#"{"version":2,"bindings":[]}"#,r#"{"version":1,"bindings":[{"action":"nope","key":"ctrl-k"}]}"#,r#"{"version":1,"bindings":[{"action":"app::Search","key":""}]}"#,r#"{"version":1,"bindings":[{"action":"app::Search","key":"madeup-x"}]}"#] {assert!(resolve(Some(bad),cx).is_err());}
        assert_eq!(defaults.iter().find(|e| e.id=="permission::Allow").unwrap().context, "PermissionPrompt");
        assert_eq!(defaults.iter().find(|e| e.id=="chat::CyclePermissionMode").unwrap().context, "Composer > Input");
    });
}

use gpui_kit::test::TestWindowExt;
use std::{cell::Cell, rc::Rc};
struct Commands {
    focus: FocusHandle,
    scope: &'static str,
    count: Rc<Cell<usize>>,
}
impl Render for Commands {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("commands")
            .track_focus(&self.focus)
            .key_context(self.scope)
            .on_action(cx.listener(|s, _: &NewChat, _, _| s.count.set(s.count.get() + 1)))
            .on_action(
                cx.listener(|s, _: &agentkit_ui::sidebar::DeleteAction, _, _| {
                    s.count.set(s.count.get() + 1)
                }),
            )
            .on_action(
                cx.listener(|s, _: &agentkit_ui::permissions::ApprovePermission, _, _| {
                    s.count.set(s.count.get() + 1)
                }),
            )
    }
}
fn window(cx: &mut TestAppContext, scope: &'static str) -> (AnyWindowHandle, Rc<Cell<usize>>) {
    let count = Rc::new(Cell::new(0));
    let c = count.clone();
    let handle = cx.update(|cx| {
        gpui_kit::init(cx);
        let (w, v) = gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
            cx.new(|cx| Commands {
                focus: cx.focus_handle(),
                scope,
                count: c,
            })
        })
        .unwrap();
        w.update(cx, |_, w, cx| v.read(cx).focus.clone().focus(w, cx))
            .unwrap();
        w
    });
    (handle, count)
}
#[gpui_kit::test]
fn hot_reload_removes_old_custom_keys_and_invalid_file_restores_defaults(cx: &mut TestAppContext) {
    let (w, count) = window(cx, "Workbench");
    let path = PathBuf::from("keymap.json");
    cx.update(|cx| install(path.clone(), Ok(None), cx));
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.press(
            if cfg!(target_os = "macos") {
                "cmd-n"
            } else {
                "ctrl-n"
            },
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(count.get(), 1);
    cx.update(|cx| {
        install(
            path.clone(),
            Ok(Some(
                r#"{"version":1,"bindings":[{"action":"app::NewChat","key":"ctrl-k"}]}"#.into(),
            )),
            cx,
        )
    });
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.press(
            if cfg!(target_os = "macos") {
                "cmd-n"
            } else {
                "ctrl-n"
            },
            cx,
        );
        w.press("ctrl-k", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(count.get(), 2);
    cx.update(|cx| install(path, Ok(Some("invalid".into())), cx));
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.press("ctrl-k", cx);
        w.press(
            if cfg!(target_os = "macos") {
                "cmd-n"
            } else {
                "ctrl-n"
            },
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(count.get(), 3);
    assert!(cx.read(|cx| cx.global::<State>().invalid));
}
#[gpui_kit::test]
fn approval_override_is_limited_to_its_focus_scope(cx: &mut TestAppContext) {
    let (ordinary, ordinary_count) = window(cx, "Workbench");
    let (approval, count) = window(cx, "PermissionPrompt");
    cx.update(|cx| {
        install(
            PathBuf::from("keymap.json"),
            Ok(Some(
                r#"{"version":1,"bindings":[{"action":"permission::Allow","key":"ctrl-y"}]}"#
                    .into(),
            )),
            cx,
        )
    });
    for w in [ordinary, approval] {
        cx.update_window(w, |_, w, cx| {
            w.render_frame(cx);
            w.press("enter", cx);
            w.press("ctrl-y", cx);
        })
        .unwrap();
    }
    cx.run_until_parked();
    assert_eq!(ordinary_count.get(), 0);
    assert_eq!(count.get(), 1);
}
#[gpui_kit::test]
fn deleting_a_binding_also_removes_the_backspace_alias(cx: &mut TestAppContext) {
    let (w, count) = window(cx, "SessionSidebar");
    cx.update_window(w, |_, w, cx| {
        let _ = cx.new(|cx| agentkit_ui::sidebar::SessionSidebar::new(w, cx));
    })
    .unwrap();
    cx.update(|cx| {
        install(
            PathBuf::from("keymap.json"),
            Ok(Some(
                r#"{"version":1,"bindings":[{"action":"sidebar::Delete","key":null}]}"#.into(),
            )),
            cx,
        )
    });
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.press("delete", cx);
        w.press("backspace", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(count.get(), 0);
}
#[test]
fn creating_a_keymap_preserves_existing_invalid_input_and_reports_filesystem_errors() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("client/keymap.json");
    ensure_file(&path).unwrap();
    assert!(read(&path).unwrap().unwrap().contains("version"));
    std::fs::write(&path, "invalid user data").unwrap();
    ensure_file(&path).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "invalid user data");
    assert!(ensure_file(temp.path()).is_err());
}
#[gpui_kit::test]
fn removed_mode_shortcut_does_not_fall_through_to_textinput_outdent(cx: &mut TestAppContext) {
    let changes = Rc::new(Cell::new(0));
    let sink = changes.clone();
    let (w,v)=cx.update(|cx|{
    gpui_kit::init(cx);
    let session=serde_json::from_value(serde_json::json!({"id":"s","workspaceId":"w","workspaceName":"W","messages":[],"lastMessageAt":0,"isProcessing":false,"permissionMode":"safe"})).unwrap();
    let store=cx.new(|_|agentkit_store::entities::SessionStore::new(session));
    let (w,v)=gpui_kit::open_window(WindowOptions::default(),cx,|w,cx|cx.new(|cx|agentkit_ui::WorkbenchView::new("Workbench".into(),w,cx))).unwrap();
    w.update(cx,|_,w,cx|v.update(cx,|v,cx|{v.connected("W".into(),Some("model".into()),cx);v.bind_session(store,w,cx);})).unwrap();
    cx.subscribe(&v,move|_,event:&agentkit_ui::WorkbenchIntent,_|{if matches!(event,agentkit_ui::WorkbenchIntent::ModeChanged(_)){sink.set(sink.get()+1);}}).detach();(w,v)
 });
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click("composer", cx);
        w.press("shift-tab", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(changes.get(), 1);
    cx.update(|cx| {
        v.update(cx, |v, cx| v.selection_finished(cx));
        install(
            PathBuf::from("keymap.json"),
            Ok(Some(
                r#"{"version":1,"bindings":[{"action":"chat::CyclePermissionMode","key":null}]}"#
                    .into(),
            )),
            cx,
        );
    });
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.press("shift-tab", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(changes.get(), 1);
}
