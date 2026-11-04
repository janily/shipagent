#![cfg(feature = "samples")]
#![allow(clippy::unwrap_used)]
use agentkit_protocol::generated as g;
use agentkit_store::entities::SessionStore;
use agentkit_ui::{WorkbenchIntent, WorkbenchView};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, ElementInputHandler, Entity, Point, TestAppContext,
    WindowBounds, WindowOptions, px, size,
    test::{TestInput, TestWindowExt},
};
use std::{cell::RefCell, rc::Rc};
type Harness = (
    AnyWindowHandle,
    Entity<WorkbenchView>,
    Entity<SessionStore>,
    Rc<RefCell<Vec<WorkbenchIntent>>>,
);
fn open(cx: &mut TestAppContext) -> Harness {
    cx.update(|cx| {
        gpui_kit::init(cx);
        let session:g::Session=serde_json::from_value(serde_json::json!({"id":"s","workspaceId":"w","workspaceName":"W","messages":[],"lastMessageAt":0,"isProcessing":false})).unwrap();
        let store=cx.new(|_|SessionStore::new(session));
        let (w,v)=gpui_kit::open_window(WindowOptions{window_bounds:Some(WindowBounds::Windowed(Bounds{origin:Point::default(),size:size(px(1280.),px(820.))})),..Default::default()},cx,|window,cx|cx.new(|cx|WorkbenchView::new("Workbench".into(),window,cx))).unwrap();
        w.update(cx,|_,window,cx|v.update(cx,|v,cx|{v.connected("W".into(),Some("model".into()),cx);v.bind_session(store.clone(),window,cx);})).unwrap();
        let events=Rc::new(RefCell::new(Vec::new()));let sink=events.clone();
        cx.subscribe(&v,move|_,event:&WorkbenchIntent,_|sink.borrow_mut().push(event.clone())).detach();
        (w,v,store,events)
    })
}
fn sent(events: &Rc<RefCell<Vec<WorkbenchIntent>>>) -> Vec<String> {
    events
        .borrow()
        .iter()
        .filter_map(|e| {
            if let WorkbenchIntent::Submit(t) = e {
                Some(t.clone())
            } else {
                None
            }
        })
        .collect()
}

fn step(
    cx: &mut TestAppContext,
    w: AnyWindowHandle,
    f: impl FnOnce(&mut gpui_kit::Window, &mut gpui_kit::App),
) {
    cx.update_window(w, |_, window, cx| f(window, cx)).unwrap();
}
fn shortcut() -> &'static str {
    if cfg!(target_os = "macos") {
        "cmd-enter"
    } else {
        "ctrl-enter"
    }
}
fn requests(store: &Entity<SessionStore>, ids: &[&str], cx: &mut gpui_kit::App) {
    store.update(cx,|s,cx|{s.model.state.session.is_processing=true;s.model.permissions=ids.iter().map(|id|serde_json::from_value(serde_json::json!({"sessionId":"s","requestId":id,"toolName":"Bash","description":"Approval description","command":"cargo test","reason":"Read tests","impact":"Runs local commands","canAlwaysAllow":true})).unwrap()).collect();cx.notify();});
}
fn decisions(events: &Rc<RefCell<Vec<WorkbenchIntent>>>) -> Vec<(String, String, bool, bool)> {
    events
        .borrow()
        .iter()
        .filter_map(|e| {
            if let WorkbenchIntent::Permission {
                session_id,
                request_id,
                allowed,
                always,
            } = e
            {
                Some((session_id.clone(), request_id.clone(), *allowed, *always))
            } else {
                None
            }
        })
        .collect()
}
#[gpui_kit::test]
fn new_approval_focuses_and_enter_is_a_single_explicit_allow(cx: &mut TestAppContext) {
    let (w, v, s, e) = open(cx);
    step(cx, w, |window, cx| {
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("keep draft", cx);
    });
    cx.update(|cx| requests(&s, &["r1", "r2"], cx));
    step(cx, w, |window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("permission-card").focused(), Some(true));
        assert_eq!(
            window.find("permission-command").label(),
            Some("cargo test")
        );
        window.press("enter", cx);
        window.press("enter", cx);
        window.click("permission-allow", cx);
        assert_eq!(v.read(cx).current_draft(cx).unwrap().1, "keep draft");
    });
    assert_eq!(decisions(&e), [("s".into(), "r1".into(), true, false)]);
    assert!(sent(&e).is_empty());
}
#[gpui_kit::test]
fn always_requires_capability_and_escape_denies_instead_of_cancelling(cx: &mut TestAppContext) {
    let (w, _, s, e) = open(cx);
    cx.update(|cx| {
        requests(&s, &["r"], cx);
        s.update(cx, |s, cx| {
            s.model.permissions[0].can_always_allow = Some(false);
            cx.notify();
        });
    });
    step(cx, w, |window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("permission-always").is_none());
        window.press(shortcut(), cx);
    });
    assert!(decisions(&e).is_empty());
    step(cx, w, |window, cx| window.press("escape", cx));
    assert_eq!(decisions(&e), [("s".into(), "r".into(), false, false)]);
    assert!(
        !e.borrow()
            .iter()
            .any(|e| matches!(e, WorkbenchIntent::Cancel))
    );
}
#[gpui_kit::test]
fn secondary_enter_allows_session_grant_and_next_request_gets_focus(cx: &mut TestAppContext) {
    let (w, _, s, e) = open(cx);
    cx.update(|cx| requests(&s, &["one", "two"], cx));
    step(cx, w, |window, cx| {
        window.render_frame(cx);
        window.press(shortcut(), cx);
    });
    assert_eq!(decisions(&e), [("s".into(), "one".into(), true, true)]);
    cx.update(|cx| {
        s.update(cx, |s, cx| {
            s.model.permission_responded("one");
            cx.notify();
        })
    });
    step(cx, w, |window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("permission-card").focused(), Some(true));
        window.press("escape", cx);
    });
    assert_eq!(decisions(&e)[1], ("s".into(), "two".into(), false, false));
}
#[gpui_kit::test]
fn rpc_error_enables_retry_and_cached_session_cannot_duplicate_pending_decision(
    cx: &mut TestAppContext,
) {
    let (w, v, s, e) = open(cx);
    cx.update(|cx| requests(&s, &["r"], cx));
    step(cx, w, |window, cx| {
        window.render_frame(cx);
        window.click("permission-allow", cx);
    });
    let lease = cx.update(|cx| {
        s.update(cx, |s, cx| {
            let lease = s.approval.begin("r").unwrap();
            cx.notify();
            lease
        })
    });
    step(cx, w, |window, cx| {
        let mut other = s.read(cx).model.state.session.clone();
        other.id = "b".into();
        let other = cx.new(|_| SessionStore::new(other));
        v.update(cx, |v, cx| v.bind_session(other, window, cx));
        v.update(cx, |v, cx| v.bind_session(s.clone(), window, cx));
        window.render_frame(cx);
        window.press("enter", cx);
    });
    assert_eq!(decisions(&e).len(), 1);
    cx.update(|cx| {
        s.update(cx, |s, cx| {
            assert!(s.approval.finish("r", lease));
            s.approval.error = Some(("r".into(), "RPC failed".into()));
            cx.notify();
        })
    });
    step(cx, w, |window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("permission-error").label(), Some("RPC failed"));
        window.click("permission-deny", cx);
    });
    assert_eq!(decisions(&e).len(), 2);
    assert!(!decisions(&e)[1].2);
}
#[gpui_kit::test]
fn shift_tab_cycles_without_outdenting_and_execute_warning_returns_safe(cx: &mut TestAppContext) {
    let (w, v, s, e) = open(cx);
    for mode in [
        g::PermissionMode::Safe,
        g::PermissionMode::Ask,
        g::PermissionMode::AllowAll,
    ] {
        step(cx, w, |window, cx| {
            s.update(cx, |s, cx| {
                s.model.state.session.permission_mode = Some(mode);
                s.selection_pending = false;
                cx.notify();
            });
            v.update(cx, |v, cx| v.selection_finished(cx));
            window.render_frame(cx);
            window.click("composer", cx);
            if v.read(cx).current_draft(cx).unwrap().1.is_empty() {
                window.input("    unchanged", cx);
            }
            window.press("shift-tab", cx);
            assert_eq!(v.read(cx).current_draft(cx).unwrap().1, "    unchanged");
        });
    }
    let modes: Vec<_> = e
        .borrow()
        .iter()
        .filter_map(|e| {
            if let WorkbenchIntent::ModeChanged(m) = e {
                Some(*m)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        modes,
        [
            g::PermissionMode::Ask,
            g::PermissionMode::AllowAll,
            g::PermissionMode::Safe
        ]
    );
    step(cx, w, |window, cx| {
        window.render_frame(cx);
        assert!(window.find("execute-warning").visible());
        window.click("return-safe", cx);
    });
    assert!(matches!(
        e.borrow().last(),
        Some(WorkbenchIntent::ModeChanged(g::PermissionMode::Safe))
    ));
}
#[gpui_kit::test]
fn ime_preedit_blocks_mode_cycle(cx: &mut TestAppContext) {
    let (w, v, _, e) = open(cx);
    step(cx, w, |window, cx| {
        window.render_frame(cx);
        window.click("composer", cx);
        let input = v.read(cx).composer_input(cx);
        let bounds = window.find("composer").bounds();
        TestInput::new(ElementInputHandler::new(bounds, input), window).compose_text(
            None,
            "组字",
            Some(2..2),
            cx,
        );
        window.press("shift-tab", cx);
    });
    assert!(
        !e.borrow()
            .iter()
            .any(|e| matches!(e, WorkbenchIntent::ModeChanged(_)))
    );
}
#[gpui_kit::test]
fn acknowledged_receipt_is_rendered_in_owning_assistant_round(cx: &mut TestAppContext) {
    let (w, v, s, _) = open(cx);
    step(cx, w, |window, cx| {
        let session:g::Session=serde_json::from_value(serde_json::json!({"id":"s2","workspaceId":"w","workspaceName":"W","messages":[{"id":"u","role":"user","content":"run","timestamp":0},{"id":"a","role":"assistant","content":"done","timestamp":1}],"lastMessageAt":1,"isProcessing":false})).unwrap();
        let store = cx.new(|_| SessionStore::new(session));
        let request: g::PermissionRequest=serde_json::from_value(serde_json::json!({"sessionId":"s2","requestId":"receipt","toolName":"Bash","description":"Run"})).unwrap();
        store.update(cx, |s, _| {
            s.approval
                .receipts
                .push(agentkit_store::entities::permissions::PermissionReceipt {
                    request_id: request.request_id,
                    tool_name: request.tool_name,
                    allowed: false,
                    always: false,
                    anchor: s.annotations.current(),
                })
        });
        v.update(cx, |v, cx| v.bind_session(store, window, cx));
        window.render_frame(cx);
        assert!(window.find("approval-receipt:receipt").visible());
        v.update(cx, |v, cx| v.bind_session(s, window, cx));
        window.render_frame(cx);
        assert!(window.try_find("approval-receipt:receipt").is_none());
    });
}
#[gpui_kit::test]
fn hidden_prompt_does_not_steal_connection_configuration_focus(cx: &mut TestAppContext) {
    let (w, v, s, e) = open(cx);
    step(cx, w, |window, cx| {
        v.update(cx, |v, cx| v.show_configuration(window, cx))
    });
    cx.update(|cx| requests(&s, &["hidden"], cx));
    step(cx, w, |window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("permission-card").is_none());
        window.input("keep-focus", cx);
        assert!(
            window
                .within("endpoint-url")
                .find("input-state")
                .value()
                .unwrap()
                .contains("keep-focus")
        );
        window.click("back-chat", cx);
        window.render_frame(cx);
        window.press("enter", cx);
    });
    assert_eq!(decisions(&e), [("s".into(), "hidden".into(), true, false)]);
}
#[gpui_kit::test]
fn pending_other_session_has_a_warning_dot(cx: &mut TestAppContext) {
    let (w, v, s, _) = open(cx);
    step(cx, w, |window, cx| {
        let mut other = s.read(cx).model.state.session.clone();
        other.id = "other".into();
        let list = cx.new(|_| agentkit_store::entities::SessionListStore::default());
        let current = s.read(cx).model.state.session.clone();
        list.update(cx, |list, _| {
            list.model.sessions = vec![current, other];
            list.model.awaiting.insert("other".into(), vec!["r".into()]);
            list.model
                .awaiting
                .insert("s".into(), vec!["active".into()]);
        });
        v.update(cx, |v, cx| v.bind_list(list, cx));
        window.render_frame(cx);
        assert!(window.find("approval-dot:other").visible());
        assert!(window.try_find("approval-dot:s").is_none());
    });
}
#[gpui_kit::test]
fn receipt_segments_are_unique_and_a_new_user_does_not_anchor_to_old_assistant(
    cx: &mut TestAppContext,
) {
    let (w, v, _, _) = open(cx);
    step(cx, w, |window, cx| {
        let mut value = serde_json::json!({"id":"seg","workspaceId":"w","workspaceName":"W","messages":[{"id":"u","role":"user","content":"run","timestamp":0},{"id":"a1","role":"assistant","content":"first","timestamp":1},{"id":"info","role":"info","content":"boundary","timestamp":2},{"id":"a2","role":"assistant","content":"second","timestamp":3}],"lastMessageAt":3,"isProcessing":false});
        let session: g::Session = serde_json::from_value(value.clone()).unwrap();
        let store = cx.new(|_| SessionStore::new(session));
        let anchor = store.read(cx).annotations.current();
        assert_eq!(anchor.segment, 1);
        store.update(cx, |s, _| {
            s.approval
                .receipts
                .push(agentkit_store::entities::permissions::PermissionReceipt {
                    request_id: "unique".into(),
                    tool_name: "Bash".into(),
                    allowed: true,
                    always: false,
                    anchor,
                })
        });
        v.update(cx, |v, cx| v.bind_session(store, window, cx));
        window.render_frame(cx);
        assert_eq!(window.find_all("approval-receipt:unique").len(), 1);
        value["messages"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"id":"new","role":"user","content":"next","timestamp":4}));
        let session: g::Session = serde_json::from_value(value).unwrap();
        let new = SessionStore::new(session);
        let anchor = new.annotations.current();
        assert_eq!(anchor.user.as_deref(), Some("new"));
        assert_eq!(anchor.segment, 0);
        assert_eq!(new.annotations.row(&anchor), None);
    });
}
#[test]
fn stale_completion_cannot_release_a_new_permission_lease() {
    let mut s = agentkit_store::entities::permissions::ApprovalState::default();
    let first = s.begin("r").unwrap();
    assert!(s.begin("r").is_none());
    assert!(!s.finish("r", first + 1));
    assert!(s.finish("r", first));
    let second = s.begin("r").unwrap();
    assert!(!s.finish("r", first));
    assert!(s.finish("r", second));
}
#[test]
fn repeated_backend_mode_callbacks_make_one_notice_but_real_transitions_remain() {
    let mut s = agentkit_store::entities::permissions::ApprovalState::default();
    let anchor = agentkit_store::entities::annotations::RoundAnchor::default();
    assert!(s.mode_changed(None, g::PermissionMode::Ask, anchor.clone()));
    assert!(!s.mode_changed(None, g::PermissionMode::Ask, anchor.clone()));
    assert!(s.mode_changed(None, g::PermissionMode::AllowAll, anchor.clone()));
    assert!(!s.mode_changed(None, g::PermissionMode::AllowAll, anchor.clone()));
    assert!(s.mode_changed(None, g::PermissionMode::Ask, anchor));
    assert_eq!(s.modes.len(), 3);
}
