#![cfg(feature = "samples")]
#![allow(clippy::unwrap_used)]
use agentkit_protocol::generated as g;
use agentkit_store::entities::{AppState, SessionStore, WorkspaceStore};
use agentkit_ui::{
    WorkbenchIntent, WorkbenchView,
    composer::{SendKey, model_choices},
};
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
#[gpui_kit::test]
fn shortcuts_newlines_and_send_ack_preserve_newer_draft(cx: &mut TestAppContext) {
    let (w, v, _, e) = open(cx);
    step(cx, w, |window, cx| {
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("中文", cx);
        window.press("shift-enter", cx);
        window.input("second", cx);
        window.press("enter", cx);
    });
    assert_eq!(sent(&e), ["中文\nsecond"]);
    step(cx, w, |window, cx| {
        window.input(" newer", cx);
        v.update(cx, |v, cx| {
            v.submission_finished("中文\nsecond", true, window, cx)
        });
        window.render_frame(cx);
        assert_eq!(
            v.read(cx).current_draft(cx).unwrap().1,
            "中文\nsecond newer"
        );
        v.update(cx, |v, cx| v.set_send_key(SendKey::CmdEnter, cx));
        window.press("enter", cx);
        window.input("third", cx);
    });
    assert_eq!(sent(&e).len(), 1);
    let mut before = String::new();
    step(cx, w, |window, cx| {
        before = v.read(cx).current_draft(cx).unwrap().1;
        assert!(before.ends_with("\nthird"));
        window.press(shortcut(), cx);
        assert_eq!(
            v.read(cx).current_draft(cx).unwrap().1,
            before,
            "shortcut must not insert a newline"
        );
    });
    assert_eq!(sent(&e), ["中文\nsecond", &before]);
}
#[gpui_kit::test]
fn ime_preedit_blocks_both_shortcuts_and_button_until_commit(cx: &mut TestAppContext) {
    let (w, v, _, e) = open(cx);
    step(cx, w, |window, cx| {
        window.render_frame(cx);
        window.click("composer", cx);
        let input = v.read(cx).composer_input(cx);
        let bounds = window.find("composer").bounds();
        TestInput::new(ElementInputHandler::new(bounds, input), window).compose_text(
            None,
            "中文组字",
            Some(4..4),
            cx,
        );
        window.press("enter", cx);
        window.click("send", cx);
    });
    assert!(sent(&e).is_empty());
    step(cx, w, |window, cx| {
        v.update(cx, |v, cx| v.set_send_key(SendKey::CmdEnter, cx));
        window.click("composer", cx);
        window.press(shortcut(), cx);
    });
    assert!(sent(&e).is_empty());
    step(cx, w, |window, cx| {
        let input = v.read(cx).composer_input(cx);
        let bounds = window.find("composer").bounds();
        TestInput::new(ElementInputHandler::new(bounds, input), window).commit_text("中文完成", cx);
        window.click("composer", cx);
        window.press(shortcut(), cx);
    });
    assert_eq!(sent(&e), ["中文完成"]);
}
#[gpui_kit::test]
fn running_and_approval_allow_queue_and_escape_preserves_draft(cx: &mut TestAppContext) {
    let (w, v, s, e) = open(cx);
    step(cx, w, |window, cx| {
        s.update(cx, |s, cx| {
            s.model.state.session.is_processing = true;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("next", cx);
        window.press("escape", cx);
        assert_eq!(v.read(cx).current_draft(cx).unwrap().1, "next");
    });
    assert_eq!(
        e.borrow()
            .iter()
            .filter(|e| matches!(e, WorkbenchIntent::Cancel))
            .count(),
        1
    );
    step(cx, w, |window, cx| window.press("enter", cx));
    assert_eq!(sent(&e), ["next"]);
    step(cx, w, |window, cx| {
        s.update(cx,|s,cx|{s.model.permissions.push(serde_json::from_value(serde_json::json!({"requestId":"p","sessionId":"s","type":"bash","toolName":"Bash","command":"echo proof","description":"approval"})).unwrap());cx.notify();});
        v.update(cx, |v, cx| v.submission_finished("next", true, window, cx));
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("after approval", cx);
        window.click("send", cx);
    });
    assert_eq!(sent(&e), ["next", "after approval"]);
}
#[gpui_kit::test]
fn empty_history_is_session_local_and_disconnect_preserves_text(cx: &mut TestAppContext) {
    let (w, v, s, e) = open(cx);
    step(cx, w, |window, cx| {
        s.update(cx, |s, cx| {
            s.model.state.session.messages = vec![
                serde_json::from_value(
                    serde_json::json!({"id":"u","role":"user","content":"previous","timestamp":0}),
                )
                .unwrap(),
            ];
            cx.notify();
        });
        window.render_frame(cx);
        window.click("composer", cx);
        window.press("enter", cx);
        window.press("up", cx);
        assert_eq!(v.read(cx).current_draft(cx).unwrap().1, "previous");
    });
    assert!(sent(&e).is_empty());
    step(cx, w, |window, cx| {
        v.update(cx, |v, cx| v.show_error("offline".into(), true, cx));
        window.render_frame(cx);
        window.press("enter", cx);
        window.click("send", cx);
        assert_eq!(v.read(cx).current_draft(cx).unwrap().1, "previous");
    });
    assert!(sent(&e).is_empty());
    step(cx, w, |window, cx| {
        let mut session = s.read(cx).model.state.session.clone();
        session.id = "b".into();
        session.messages.clear();
        let b = cx.new(|_| SessionStore::new(session));
        v.update(cx, |v, cx| {
            v.connected("W".into(), Some("model".into()), cx);
            v.bind_session(b, window, cx);
        });
        window.render_frame(cx);
        window.click("composer", cx);
        window.press("up", cx);
        assert_eq!(v.read(cx).current_draft(cx).unwrap().1, "");
    });
}
fn connection(slug: &str, auth: bool) -> g::LlmConnectionWithStatus {
    serde_json::from_value(serde_json::json!({"slug":slug,"name":slug,"providerType":"pi_compat","authType":"none","createdAt":0,"isAuthenticated":auth,"defaultModel":"model","models":["model","second","model"]})).unwrap()
}
#[gpui_kit::test]
fn authenticated_model_menu_emits_session_intent_and_rejects_expired_connection(
    cx: &mut TestAppContext,
) {
    let (w, v, _, e) = open(cx);
    let app = cx.update(|cx| {
        let app = AppState {
            connections: vec![connection("valid", true), connection("expired", false)],
            ..Default::default()
        };
        assert_eq!(model_choices(&app.connections).len(), 2);
        cx.new(|_| app)
    });
    step(cx, w, |window, cx| {
        let workspace = cx.new(|_| WorkspaceStore::default());
        v.update(cx, |v, cx| v.bind_context(app.clone(), workspace, cx));
        window.render_frame(cx);
        window.click("composer-model", cx);
        window.within("popup-menu").click(2usize, cx);
    });
    assert!(e.borrow().iter().any(|e|matches!(e,WorkbenchIntent::ModelChanged{connection,model} if connection=="valid" && model=="second")));
    step(cx, w, |window, cx| {
        v.update(cx, |v, cx| v.selection_finished(cx));
        app.update(cx, |a, cx| {
            a.connections[0].is_authenticated = false;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("blocked", cx);
        window.click("send", cx);
    });
    assert!(sent(&e).is_empty());
}

#[gpui_kit::test]
fn toolbar_choices_and_popup_escape_do_not_cancel_a_running_turn(cx: &mut TestAppContext) {
    let (w, v, s, e) = open(cx);
    step(cx, w, |window, cx| {
        s.update(cx, |s, cx| {
            s.model.state.session.is_processing = true;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("composer-thinking", cx);
        window.within("popup-menu").press("escape", cx);
    });
    assert!(
        !e.borrow()
            .iter()
            .any(|e| matches!(e, WorkbenchIntent::Cancel))
    );
    step(cx, w, |window, cx| {
        window.click("composer-thinking", cx);
        window.within("popup-menu").click(3usize, cx);
    });
    assert!(
        e.borrow()
            .iter()
            .any(|e| matches!(e, WorkbenchIntent::ThinkingChanged(g::ThinkingLevel::High)))
    );
    step(cx, w, |window, cx| {
        v.update(cx, |v, cx| v.selection_finished(cx));
        window.render_frame(cx);
        window.click("composer-mode", cx);
        window.within("popup-menu").click(1usize, cx);
    });
    assert!(
        e.borrow()
            .iter()
            .any(|e| matches!(e, WorkbenchIntent::ModeChanged(g::PermissionMode::Ask)))
    );
    step(cx, w, |window, cx| {
        v.update(cx, |v, cx| v.selection_finished(cx));
        window.click("composer-send-key", cx);
        window.within("popup-menu").click(1usize, cx);
    });
    assert!(
        e.borrow()
            .iter()
            .any(|e| matches!(e, WorkbenchIntent::SendKeyChanged(SendKey::CmdEnter)))
    );
}
#[gpui_kit::test]
fn session_selection_lock_survives_roundtrip_and_disconnect_rejection_unlocks(
    cx: &mut TestAppContext,
) {
    let (w, v, a, e) = open(cx);
    let b = cx.update(|cx| {
        let mut s = a.read(cx).model.state.session.clone();
        s.id = "b".into();
        cx.new(|_| SessionStore::new(s))
    });
    step(cx, w, |window, cx| {
        a.update(cx, |s, cx| {
            s.selection_pending = true;
            cx.notify();
        });
        v.update(cx, |v, cx| v.bind_session(b, window, cx));
        v.update(cx, |v, cx| v.bind_session(a.clone(), window, cx));
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("preserved", cx);
        window.press("enter", cx);
        window.click("composer-thinking", cx);
        assert!(window.try_find("popup-menu").is_none());
    });
    assert!(sent(&e).is_empty());
    step(cx, w, |window, cx| {
        a.update(cx, |s, cx| {
            s.selection_pending = false;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("composer-thinking", cx);
        window.within("popup-menu").click(3usize, cx);
    });
    assert!(
        e.borrow()
            .iter()
            .any(|e| matches!(e, WorkbenchIntent::ThinkingChanged(_)))
    );
    // Simulate rejection before a transport-bound RPC could be started.
    step(cx, w, |window, cx| {
        v.update(cx, |v, cx| {
            v.show_error("unavailable".into(), true, cx);
            v.transport_recovered(false, cx);
        });
        window.render_frame(cx);
        window.click("composer", cx);
        window.press("enter", cx);
    });
    assert_eq!(sent(&e), ["preserved"]);
}
#[gpui_kit::test]
fn workspace_connection_and_model_override_global_startup_model(cx: &mut TestAppContext) {
    let (w, v, _, e) = open(cx);
    step(cx, w, |window, cx| {
        let mut global = connection("global", true);
        global.is_default = Some(true);
        let mut workspace_connection = connection("workspace", true);
        workspace_connection.default_model = Some("workspace-model".into());
        workspace_connection.models = Some(vec![
            g::LlmConnectionWithStatusModelsItem::String("workspace-model".into()),
            g::LlmConnectionWithStatusModelsItem::String("override".into()),
        ]);
        let app = cx.new(|_| AppState {
            connections: vec![global, workspace_connection],
            ..Default::default()
        });
        let settings: g::WorkspaceSettings = serde_json::from_value(
            serde_json::json!({"defaultLlmConnection":"workspace","model":"override"}),
        )
        .unwrap();
        let workspace = cx.new(|_| WorkspaceStore {
            settings: Some(settings),
            ..Default::default()
        });
        v.update(cx, |v, cx| v.bind_context(app, workspace, cx));
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("use workspace", cx);
        window.press("enter", cx);
    });
    assert_eq!(sent(&e), ["use workspace"]);
}
#[gpui_kit::test]
fn removed_escape_shortcut_does_not_cancel_via_legacy_textinput_action(cx: &mut TestAppContext) {
    let (w, v, s, e) = open(cx);
    step(cx, w, |w, cx| {
        s.update(cx, |s, cx| {
            s.model.state.session.is_processing = true;
            cx.notify();
        });
        v.update(cx, |v, cx| v.set_escape_stops(false, cx));
        w.render_frame(cx);
        w.click("composer", cx);
        w.input("draft", cx);
        w.press("escape", cx);
    });
    assert!(
        !e.borrow()
            .iter()
            .any(|e| matches!(e, WorkbenchIntent::Cancel))
    );
    step(cx, w, |w, cx| {
        v.update(cx, |v, cx| v.set_escape_stops(true, cx));
        w.render_frame(cx);
        w.press("escape", cx);
    });
    assert_eq!(
        e.borrow()
            .iter()
            .filter(|e| matches!(e, WorkbenchIntent::Cancel))
            .count(),
        1
    );
}
