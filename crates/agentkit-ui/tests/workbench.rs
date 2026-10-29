#![allow(clippy::unwrap_used)]
use agentkit_protocol::prototype::{Session, SessionEvent};
use agentkit_store::ChatState;
use agentkit_ui::{WorkbenchIntent, WorkbenchView};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, Point, TestAppContext, WindowBounds,
    WindowOptions, px, size, test::TestWindowExt,
};
use std::{cell::RefCell, rc::Rc};
fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<WorkbenchView>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        let (window, view) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(1280.), px(820.)),
                })),
                ..WindowOptions::default()
            },
            cx,
            |window, cx| cx.new(|cx| WorkbenchView::new("Workbench".into(), window, cx)),
        )
        .unwrap();
        (window, view)
    })
}
#[gpui_kit::test]
fn composer_emits_submit_and_stop_using_native_controls(cx: &mut TestAppContext) {
    let (window, view) = open(cx);
    let intents = Rc::new(RefCell::new(Vec::new()));
    cx.update(|cx| {
        let intents = intents.clone();
        cx.subscribe(&view, move |_, intent: &WorkbenchIntent, _| match intent {
            WorkbenchIntent::Submit(text) => intents.borrow_mut().push(text.clone()),
            WorkbenchIntent::Cancel => intents.borrow_mut().push("STOP".into()),
            _ => {}
        })
        .detach();
    });
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.connected("default".into(), Some("fixture-model".into()), cx);
            view.show_session(
                ChatState::new(Session {
                    id: "s".into(),
                    ..Session::default()
                }),
                cx,
            );
        });
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("中文 test", cx);
        window.click("send", cx);
    })
    .unwrap();
    cx.update_window(window, |_, window, cx| {
        let mut state = ChatState::new(Session {
            id: "s".into(),
            ..Session::default()
        });
        state.begin_send("中文 test".into(), "u".into());
        view.update(cx, |view, cx| view.show_session(state, cx));
        window.render_frame(cx);
        window.click("stop", cx);
    })
    .unwrap();
    assert_eq!(*intents.borrow(), ["中文 test", "STOP"]);
}
#[gpui_kit::test]
fn streamed_text_tool_result_and_final_answer_reach_the_window(cx: &mut TestAppContext) {
    let (window, view) = open(cx);
    let mut state = ChatState::new(Session {
        id: "s".into(),
        ..Session::default()
    });
    state.begin_send("read".into(), "u".into());
    for value in [
        serde_json::json!({"type":"text_delta","sessionId":"s","turnId":"t","delta":"流式"}),
        serde_json::json!({"type":"text_delta","sessionId":"s","turnId":"t","delta":"答案"}),
    ] {
        state.apply(&serde_json::from_value::<SessionEvent>(value).unwrap());
    }
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.connected("default".into(), Some("model".into()), cx);
            view.show_session(state.clone(), cx);
        });
        window.render_frame(cx);
        assert_eq!(window.find(("message", 1usize)).label(), Some("流式答案"));
    })
    .unwrap();
    state.apply(&serde_json::from_value(serde_json::json!({"type":"text_complete","sessionId":"s","turnId":"t","text":"最终答案","messageId":"final"})).unwrap());
    for value in [
        serde_json::json!({"type":"tool_start","sessionId":"s","toolUseId":"read","toolName":"Read","toolInput":{"path":"proof.txt"}}),
        serde_json::json!({"type":"tool_result","sessionId":"s","toolUseId":"read","result":"REAL_TOOL_RESULT","isError":false}),
        serde_json::json!({"type":"complete","sessionId":"s"}),
    ] {
        state.apply(&serde_json::from_value(value).unwrap());
    }

    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| view.show_session(state, cx));
        window.render_frame(cx);
        assert_eq!(window.find(("message", 1usize)).label(), Some("最终答案"));
        assert!(window.try_find("stop").is_none());
        window.click("tool:tool:read", cx);
        window.render_frame(cx);
        assert!(window.try_find("tool:tool:read").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_failed_send_keeps_the_draft_available_for_retry(cx: &mut TestAppContext) {
    let (window, view) = open(cx);
    let sent = Rc::new(RefCell::new(Vec::new()));
    cx.update(|cx| {
        let sent = sent.clone();
        cx.subscribe(&view, move |_, intent: &WorkbenchIntent, _| {
            if let WorkbenchIntent::Submit(text) = intent {
                sent.borrow_mut().push(text.clone());
            }
        })
        .detach();
    });
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.connected("default".into(), Some("model".into()), cx);
            view.show_session(
                ChatState::new(Session {
                    id: "s".into(),
                    ..Session::default()
                }),
                cx,
            );
        });
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("preserve my draft", cx);
        window.click("send", cx);
    })
    .unwrap();
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.submission_finished("preserve my draft", false, window, cx);
            view.show_error("offline".into(), false, cx);
        });
        window.render_frame(cx);
        window.click("send", cx);
    })
    .unwrap();
    assert_eq!(*sent.borrow(), ["preserve my draft", "preserve my draft"]);
}

#[gpui_kit::test]
fn permission_buttons_emit_one_explicit_decision(cx: &mut TestAppContext) {
    let (window, view) = open(cx);
    let decisions = Rc::new(RefCell::new(Vec::new()));
    cx.update(|cx| {
        let decisions = decisions.clone();
        cx.subscribe(&view, move |_, intent: &WorkbenchIntent, _| {
            if let WorkbenchIntent::Permission {
                request_id,
                allowed,
                always,
            } = intent
            {
                decisions
                    .borrow_mut()
                    .push((request_id.clone(), *allowed, *always));
            }
        })
        .detach();
    });
    let mut state = ChatState::new(Session {
        id: "s".into(),
        ..Session::default()
    });
    state.begin_send("run".into(), "u".into());
    state.apply(&serde_json::from_value(serde_json::json!({"type":"permission_request","sessionId":"s","request":{"sessionId":"s","requestId":"r","toolName":"bash","description":"Allow command?","canAlwaysAllow":true}})).unwrap());
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.connected("default".into(), Some("model".into()), cx);
            view.show_session(state, cx);
        });
        window.render_frame(cx);
        window.click("permission-deny", cx);
        window.click("permission-deny", cx);
    })
    .unwrap();
    assert_eq!(*decisions.borrow(), [("r".to_string(), false, false)]);
}

#[gpui_kit::test]
fn enter_during_session_loading_does_not_consume_or_submit_the_draft(cx: &mut TestAppContext) {
    let (window, view) = open(cx);
    let sent = Rc::new(RefCell::new(Vec::new()));
    cx.update(|cx| {
        let sent = sent.clone();
        cx.subscribe(&view, move |_, intent: &WorkbenchIntent, _| {
            if let WorkbenchIntent::Submit(text) = intent {
                sent.borrow_mut().push(text.clone());
            }
        })
        .detach();
    });
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.connected("default".into(), Some("model".into()), cx);
            view.show_session(
                ChatState::new(Session {
                    id: "s".into(),
                    ..Session::default()
                }),
                cx,
            );
        });
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("keep draft", cx);
        view.update(cx, |view, cx| view.set_loading(cx));
        window.render_frame(cx);
        window.press("enter", cx);
        assert!(sent.borrow().is_empty());
        view.update(cx, |view, cx| {
            view.show_error("load failed".into(), false, cx)
        });
        window.render_frame(cx);
        window.click("send", cx);
    })
    .unwrap();
    assert_eq!(*sent.borrow(), ["keep draft"]);
}
