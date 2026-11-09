#![cfg(feature = "samples")]
#![allow(clippy::unwrap_used)]
use agentkit_protocol::generated as g;
use agentkit_store::{
    core::{
        session::SessionStoreEvent,
        turns::{AssistantTurn, Turn},
    },
    entities::SessionStore,
};
use agentkit_ui::transcript::{ChatIntent, ChatView, MarkdownView};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, Point, TestAppContext, WindowBounds,
    WindowOptions, px, size, test::TestWindowExt,
};
use serde_json::json;
use std::{cell::RefCell, rc::Rc};
fn session(messages: Vec<serde_json::Value>) -> g::Session {
    serde_json::from_value(json!({"id":"s","workspaceId":"w","workspaceName":"W","messages":messages,"lastMessageAt":100,"isProcessing":false})).unwrap()
}
fn message(id: &str, role: &str, text: &str, n: i64) -> serde_json::Value {
    json!({"id":id,"role":role,"content":text,"timestamp":n})
}
fn open(
    cx: &mut TestAppContext,
    data: g::Session,
) -> (AnyWindowHandle, Entity<ChatView>, Entity<SessionStore>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        let store = cx.new(|_| SessionStore::new(data));
        let (window, view) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(1100.), px(700.)),
                })),
                ..Default::default()
            },
            cx,
            |_, cx| cx.new(ChatView::new),
        )
        .unwrap();
        view.update(cx, |v, cx| v.bind(store.clone(), cx));
        (window, view, store)
    })
}
fn turn(complete: bool, streaming: bool, count: usize) -> AssistantTurn {
    serde_json::from_value(json!({"type":"assistant","turnId":"t","timestamp":1,"isComplete":complete,"isStreaming":streaming,"activities":(0..count).map(|i|json!({"id":format!("tool{i}"),"type":"tool","status":if complete{"completed"}else{"running"},"timestamp":2+i,"toolName":"Read","elapsedSeconds":1.2})).collect::<Vec<_>>(),"response":if streaming||complete{json!({"text":"Final **Markdown**","isStreaming":streaming,"messageId":"a"})}else{serde_json::Value::Null}})).unwrap()
}
fn replace(store: &Entity<SessionStore>, turn: AssistantTurn, cx: &mut gpui_kit::App) {
    store.update(cx, |s, cx| {
        s.model.turns = vec![Turn::Assistant(turn)];
        cx.emit(SessionStoreEvent::TurnsChanged { from_turn: 0 });
        cx.notify();
    });
}
#[gpui_kit::test]
fn phases_and_completed_activity_folding_preserve_the_response(cx: &mut TestAppContext) {
    let (w, _, store) = open(
        cx,
        session(vec![message("a", "assistant", "Final **Markdown**", 1)]),
    );
    cx.update(|cx| replace(&store, turn(false, false, 0), cx));
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("turn-thinking").is_some());
    })
    .unwrap();
    cx.update(|cx| replace(&store, turn(false, false, 1), cx));
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("tool:tool0").is_some());
        assert!(window.try_find("turn-thinking").is_none());
    })
    .unwrap();
    cx.update(|cx| {
        let mut awaiting = turn(false, false, 1);
        awaiting.activities[0].status = agentkit_store::core::turns::ActivityStatus::Completed;
        replace(&store, awaiting, cx);
    });
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("turn-thinking").is_some());
    })
    .unwrap();
    cx.update(|cx| replace(&store, turn(false, true, 4), cx));
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("turn-streaming").is_some());
        assert!(window.try_find("turn-thinking").is_none());
        assert!(window.try_find("tool:tool0").is_some());
    })
    .unwrap();
    cx.update(|cx| replace(&store, turn(true, false, 4), cx));
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("tool:tool0").is_none());
        assert_eq!(
            window.find(("message", 0usize)).label(),
            Some("Final **Markdown**")
        );
        window.click("activities:assistant:msg:a", cx);
        window.render_frame(cx);
        assert!(window.try_find("tool:tool0").is_some());
    })
    .unwrap();
}
#[gpui_kit::test]
fn cache_is_bounded_and_recreates_old_history_after_scrolling(cx: &mut TestAppContext) {
    let data = session(
        (0..1000)
            .map(|i| message(&format!("a{i}"), "assistant", &format!("Answer {i}"), i))
            .collect(),
    );
    let (w, view, _) = open(cx, data);
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        assert!(view.read(cx).cache_sizes().0 <= 401);
        let scroll = view.read(cx).scroll_state().clone();
        scroll.update(cx, |s, cx| {
            s.scroll_to_item(0, cx);
        });
    })
    .unwrap();
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        assert_eq!(window.find(("message", 0usize)).label(), Some("Answer 0"));
        assert!(view.read(cx).cache_sizes().0 <= 401);
        assert!(!view.read(cx).scroll_state().read(cx).is_following_tail());
    })
    .unwrap();
}
#[gpui_kit::test]
fn typed_errors_expose_explicit_actions_and_expand_technical_details(cx: &mut TestAppContext) {
    let mut error = message("e", "error", "Rate limit reached", 2);
    error["errorTitle"] = json!("Rate limit");
    error["errorCanRetry"] = json!(true);
    error["errorDetails"] = json!(["request detail"]);
    error["errorActions"] = json!([{"key":"settings","label":"Configure","action":"settings"}]);
    let (w, view, _) = open(
        cx,
        session(vec![message("u", "user", "Original request", 1), error]),
    );
    let events = Rc::new(RefCell::new(Vec::new()));
    cx.update(|cx| {
        let events = events.clone();
        cx.subscribe(&view, move |_, event: &ChatIntent, _| {
            events.borrow_mut().push(event.clone())
        })
        .detach();
    });
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("error:e").is_some());
        window.click("error-action:e:retry", cx);
        window.click("error-action:e:settings", cx);
        window.click("error-details:e", cx);
        window.render_frame(cx);
        assert!(window.try_find("error-technical-details").is_some());
    })
    .unwrap();
    assert!(
        matches!(&events.borrow()[0],ChatIntent::ErrorAction{message_id,key} if message_id=="e"&&key=="retry")
    );
    assert_eq!(events.borrow().len(), 2);
}
#[gpui_kit::test]
fn persisted_user_actions_emit_canonical_ids_and_queue_state_is_visible(cx: &mut TestAppContext) {
    let mut user = message("u", "user", "Queued draft", 1);
    user["isPending"] = json!(false);
    user["isQueued"] = json!(true);
    let (w, view, _) = open(cx, session(vec![user]));
    let events = Rc::new(RefCell::new(Vec::new()));
    cx.update(|cx| {
        let events = events.clone();
        cx.subscribe(&view, move |_, event: &ChatIntent, _| {
            events.borrow_mut().push(event.clone())
        })
        .detach();
    });
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click("copy:u", cx);
        window.click("branch:u", cx);
    })
    .unwrap();
    assert!(matches!(&events.borrow()[0],ChatIntent::CopyMessage(id) if id=="u"));
    assert!(matches!(&events.borrow()[1],ChatIntent::BranchFrom(id) if id=="u"));
}
#[gpui_kit::test]
fn markdown_document_identity_survives_streaming_and_final_correction(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        let view = cx.new(|cx| {
            MarkdownView::new(
                "# Heading\n\n```rust\nfn main() {}\n```\n\n| A | B |\n| - | - |\n| 一 | 二 |\n",
                true,
                cx,
            )
        });
        let id = view.read(cx).text_state().entity_id();
        view.update(cx, |v, cx| v.append("\n[docs](https://example.com)", cx));
        view.update(cx, |v, cx| v.replace("Final **中文**", false, cx));
        assert_eq!(view.read(cx).text_state().entity_id(), id);
        assert_eq!(view.read(cx).text(), "Final **中文**");
        let state = view.read(cx).text_state().clone();
        state.update(cx, |s, cx| s.select_all(cx));
        assert_eq!(state.read(cx).selected_text(), "Final 中文\n");
    });
}
#[gpui_kit::test]
fn markdown_links_emit_an_intent_instead_of_opening_the_os_directly(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let (w, _) = cx.update(|cx| {
        gpui_kit::init(cx);
        let (w, view) = gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
            cx.new(|cx| MarkdownView::new("[docs](https://example.com)", false, cx))
        })
        .unwrap();
        let events = events.clone();
        cx.subscribe(&view, move |_, event: &ChatIntent, _| {
            events.borrow_mut().push(event.clone())
        })
        .detach();
        (w, view)
    });
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click_at("markdown-document", gpui_kit::point(px(10.), px(10.)), cx);
    })
    .unwrap();
    assert!(
        matches!(&events.borrow()[0],ChatIntent::LinkClicked(url) if url=="https://example.com")
    );
}
#[test]
fn copied_turn_markdown_does_not_duplicate_a_promoted_intermediate() {
    let mut t = turn(true, false, 0);
    let mut a:agentkit_store::core::turns::Activity=serde_json::from_value(json!({"id":"a","type":"intermediate","status":"completed","timestamp":1,"content":"Final **Markdown**"})).unwrap();
    t.activities.push(a.clone());
    assert_eq!(
        agentkit_ui::transcript::turn_markdown(&t),
        "Final **Markdown**"
    );
    a.id = "earlier".into();
    a.content = Some("Earlier commentary".into());
    t.activities.insert(0, a);
    assert_eq!(
        agentkit_ui::transcript::turn_markdown(&t),
        "Earlier commentary\n\nFinal **Markdown**"
    );
}

#[gpui_kit::test]
fn loaded_plan_markdown_remeasures_the_virtual_row(cx: &mut TestAppContext) {
    let (w, _, store) = open(cx, session(vec![message("plan", "plan", "/plan.md", 1)]));
    let before = cx
        .update_window(w, |_, w, cx| {
            w.render_frame(cx);
            w.render_frame(cx);
            w.find(("message", 0usize)).bounds().size.height
        })
        .unwrap();
    cx.update(|cx| {
        store.update(cx, |s, cx| {
            s.plan.content.insert(
                "plan".into(),
                Ok((0..12)
                    .map(|i| format!("{}. Verify plan step.\n\n", i + 1))
                    .collect()),
            );
            cx.emit(SessionStoreEvent::TurnsChanged { from_turn: 0 });
            cx.notify();
        })
    });
    let after = cx
        .update_window(w, |_, w, cx| {
            w.render_frame(cx);
            w.render_frame(cx);
            w.find(("message", 0usize)).bounds().size.height
        })
        .unwrap();
    assert!(
        after > before,
        "loaded Markdown must grow the measured plan row"
    );
}
