#![cfg(feature = "samples")]
#![allow(clippy::unwrap_used)]
use agentkit_protocol::generated as g;
use agentkit_store::{core::session::SessionStoreEvent, entities::SessionStore};
use agentkit_ui::{
    tools::{FullToolEditor, ToolContent, ToolOutputView},
    transcript::{ChatIntent, ChatView},
};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, Point, TestAppContext, WindowBounds,
    WindowOptions, px, size, test::TestWindowExt,
};
use serde_json::json;
use std::{cell::RefCell, rc::Rc};
fn message(id: &str, name: &str, result: &str, input: serde_json::Value) -> serde_json::Value {
    json!({"id":id,"role":"tool","content":"","toolName":name,"toolUseId":id,"toolStatus":"completed","toolResult":result,"toolInput":input,"timestamp":1})
}
fn open(
    cx: &mut TestAppContext,
    messages: Vec<serde_json::Value>,
) -> (AnyWindowHandle, Entity<ChatView>, Entity<SessionStore>) {
    cx.update(|cx|{
    gpui_kit::init(cx);let data:g::Session=serde_json::from_value(json!({"id":"s","workspaceId":"w","workspaceName":"W","messages":messages,"lastMessageAt":2,"isProcessing":false})).unwrap();let store=cx.new(|_|SessionStore::new(data));
    let(w,view)=gpui_kit::open_window(WindowOptions{window_bounds:Some(WindowBounds::Windowed(Bounds{origin:Point::default(),size:size(px(1200.),px(900.))})),..Default::default()},cx,|_,cx|cx.new(ChatView::new)).unwrap();view.update(cx,|v,cx|v.bind(store.clone(),cx));(w,view,store)
})
}
#[gpui_kit::test]
fn collapsed_result_has_no_editor_and_expansion_is_read_only(cx: &mut TestAppContext) {
    let (w, _, _) = open(
        cx,
        vec![message(
            "read",
            "Read",
            "fn main() {}\n",
            json!({"path":"main.rs"}),
        )],
    );
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("tool-output:read").is_none());
        window.click("tool:read", cx);
        window.render_frame(cx);
        let editor = window.within("tool-output:read").find("input-state");
        assert_eq!(editor.value(), Some("fn main() {}\n"));
        window.within("tool-output:read").click("input-state", cx);
        window.input("attempted edit", cx);
        window.render_frame(cx);
        assert_eq!(
            window
                .within("tool-output:read")
                .find("input-state")
                .value(),
            Some("fn main() {}\n")
        );
        window.click("tool:read", cx);
        window.render_frame(cx);
        assert!(window.try_find("tool-output:read").is_none());
    })
    .unwrap();
}
#[gpui_kit::test]
fn full_result_intent_does_not_contain_the_truncated_payload(cx: &mut TestAppContext) {
    let text = (0..100_000)
        .map(|i| format!("line{i}\n"))
        .collect::<String>();
    let (w, view, _) = open(
        cx,
        vec![message(
            "large",
            "Read",
            &text,
            json!({"path":"output.txt","content":"中文".repeat(1000)}),
        )],
    );
    let events = Rc::new(RefCell::new(Vec::new()));
    cx.update(|cx| {
        let events = events.clone();
        cx.subscribe(&view, move |_, intent: &ChatIntent, _| {
            events.borrow_mut().push(intent.clone())
        })
        .detach();
    });
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click("tool:large", cx);
        window.render_frame(cx);
        window
            .within("tool-output:large")
            .click("input-full:large:content", cx);
        window
            .within("tool-output:large")
            .click("tool-input-full", cx);
        window
            .within("tool-output:large")
            .click("tool-result-full", cx);
    })
    .unwrap();
    assert!(
        matches!(&events.borrow()[0],ChatIntent::OpenToolContent{activity_id,content:ToolContent::Field(key)} if activity_id=="large"&&key=="content")
    );
    assert!(matches!(
        &events.borrow()[1],
        ChatIntent::OpenToolContent {
            content: ToolContent::Input,
            ..
        }
    ));
    assert!(
        matches!(&events.borrow()[2],ChatIntent::OpenToolContent{activity_id,content:ToolContent::Result} if activity_id=="large")
    );
}
#[gpui_kit::test]
fn full_editor_keeps_all_hundred_thousand_lines_and_rejects_typing(cx: &mut TestAppContext) {
    let text = (0..100_000)
        .map(|i| format!("line{i}\n"))
        .collect::<String>();
    let (w, editor) = cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| FullToolEditor::new(text, "plaintext", window, cx))
        })
        .unwrap()
    });
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.within("tool-full-editor").click("input-state", cx);
        window.input("MUTATION", cx);
        window.render_frame(cx);
        let state = editor.read(cx).state().clone();
        assert_eq!(state.read(cx).value().lines().count(), 100_000);
        assert!(!state.read(cx).value().contains("MUTATION"));
    })
    .unwrap();
}
#[gpui_kit::test]
fn expanded_result_refreshes_from_canonical_updates(cx: &mut TestAppContext) {
    let (w, _, store) = open(
        cx,
        vec![message("read", "Read", "old", json!({"path":"result.rs"}))],
    );
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click("tool:read", cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.update(|cx| {
        store.update(cx, |s, cx| {
            let mut data = s.model.state.session.clone();
            data.messages[0].tool_result = Some("new authoritative result".into());
            s.model = agentkit_store::core::session::SessionModel::new(data);
            cx.emit(SessionStoreEvent::TurnsChanged { from_turn: 0 });
            cx.notify();
        })
    });
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        assert_eq!(
            window
                .within("tool-output:read")
                .find("input-state")
                .value(),
            Some("new authoritative result")
        );
    })
    .unwrap();
}
#[gpui_kit::test]
fn child_tools_are_grouped_indented_and_can_be_collapsed(cx: &mut TestAppContext) {
    let parent = message("parent", "Agent", "finished", json!({}));
    let mut child = message("child", "Read", "child result", json!({}));
    child["parentToolUseId"] = json!("parent");
    child["timestamp"] = json!(2);
    let (w, _, _) = open(cx, vec![parent, child]);
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        let parent = window.find("tool-card:parent").bounds();
        let child = window.find("tool-card:child").bounds();
        assert!(child.origin.x > parent.origin.x);
        window.click("tool-children:parent", cx);
        window.render_frame(cx);
        assert!(window.try_find("tool:child").is_none());
        window.click("tool-children:parent", cx);
        window.render_frame(cx);
        assert!(window.try_find("tool:child").is_some());
    })
    .unwrap();
}
#[gpui_kit::test]
fn malformed_diff_shows_a_safe_fallback_and_full_input_action(cx: &mut TestAppContext) {
    let (w, _, _) = open(
        cx,
        vec![message(
            "edit",
            "Edit",
            "backend result",
            json!({"path":"a.rs","diff":"not a patch"}),
        )],
    );
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click("tool:edit", cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .within("tool-output:edit")
                .try_find("tool-diff")
                .is_none()
        );
        assert!(
            window
                .within("tool-output:edit")
                .try_find("tool-input-full")
                .is_some()
        );
    })
    .unwrap();
}
#[gpui_kit::test]
fn edit_inputs_produce_a_real_native_diff_and_preserve_its_statistics(cx: &mut TestAppContext) {
    let activity:agentkit_store::core::turns::Activity=serde_json::from_value(json!({"id":"edit","type":"tool","status":"completed","timestamp":1,"toolName":"Edit","content":"Edited successfully","toolInput":{"path":"a.rs","oldText":"fn old() {}\n","newText":"fn new() {}\n"}})).unwrap();
    let (w, view) = cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| ToolOutputView::new(&activity, window, cx))
        })
        .unwrap()
    });
    cx.run_until_parked();
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        let diff = view.read(cx).diff_state().unwrap().clone();
        assert_eq!(diff.read(cx).files()[0].additions(), 1);
        assert_eq!(diff.read(cx).files()[0].deletions(), 1);
        assert!(window.try_find("tool-diff").is_some());
    })
    .unwrap();
}
#[gpui_kit::test]
fn very_deep_groups_cap_indentation_and_orphans_and_cycles_remain_visible(cx: &mut TestAppContext) {
    let mut messages = Vec::new();
    for i in 0..8 {
        let mut m = message(
            &format!("d{i}"),
            if i == 0 { "Agent" } else { "Read" },
            "done",
            json!({}),
        );
        m["timestamp"] = json!(i + 1);
        if i > 0 {
            m["parentToolUseId"] = json!(format!("d{}", i - 1));
        }
        messages.push(m);
    }
    let (w, _, store) = open(cx, messages);
    let key = cx.update(|cx| match &store.read(cx).model.turns[0] {
        agentkit_store::core::turns::Turn::Assistant(t) => {
            agentkit_store::core::turns::get_assistant_turn_ui_key(t, 0)
        }
        _ => panic!("assistant"),
    });
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("activities:{key}"), cx);
        window.render_frame(cx);
        assert_eq!(
            window.find("tool-card:d4").bounds().origin.x,
            window.find("tool-card:d7").bounds().origin.x
        );
    })
    .unwrap();
    let mut a = message("a", "Agent", "done", json!({}));
    a["parentToolUseId"] = json!("b");
    let mut b = message("b", "Agent", "done", json!({}));
    b["parentToolUseId"] = json!("a");
    let mut orphan = message("orphan", "Read", "done", json!({}));
    orphan["parentToolUseId"] = json!("missing");
    let (w, _, _) = open(cx, vec![a, b, orphan]);
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("tool:a").is_some());
        assert!(window.try_find("tool:b").is_some());
        assert!(window.try_find("tool:orphan").is_some());
        assert!(window.try_find("tool:continued:missing").is_some());
    })
    .unwrap();
}
#[gpui_kit::test]
fn an_unrelated_later_turn_keeps_the_historical_editor_focused(cx: &mut TestAppContext) {
    let answer = json!({"id":"a","role":"assistant","content":"done","timestamp":2});
    let (w, _, store) = open(
        cx,
        vec![
            message("read", "Read", "historical output", json!({"path":"a.rs"})),
            answer,
        ],
    );
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click("tool:read", cx);
        window.render_frame(cx);
        window.within("tool-output:read").click("input-state", cx);
        window.render_frame(cx);
        assert_eq!(
            window
                .within("tool-output:read")
                .find("input-state")
                .focused(),
            Some(true)
        );
    })
    .unwrap();
    cx.update(|cx| {
        store.update(cx, |s, cx| {
            let mut data = s.model.state.session.clone();
            data.messages.push(
                serde_json::from_value(
                    json!({"id":"u2","role":"user","content":"later question","timestamp":3}),
                )
                .unwrap(),
            );
            data.messages.push(
                serde_json::from_value(
                    json!({"id":"a2","role":"assistant","content":"later answer","timestamp":4}),
                )
                .unwrap(),
            );
            s.model = agentkit_store::core::session::SessionModel::new(data);
            cx.emit(SessionStoreEvent::TurnsChanged { from_turn: 0 });
            cx.notify();
        })
    });
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window
                .within("tool-output:read")
                .find("input-state")
                .focused(),
            Some(true)
        );
    })
    .unwrap();
}
