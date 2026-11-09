#![cfg(feature = "samples")]
#![allow(clippy::unwrap_used)]
use agentkit_store::entities::SessionStore;
use agentkit_ui::transcript::{ChatIntent, PlanCard};
use gpui_kit::{
    AppContext, Bounds, Point, TestAppContext, WindowBounds, WindowOptions, px, size,
    test::TestWindowExt,
};
use std::{cell::RefCell, rc::Rc};
#[gpui_kit::test]
fn canonical_plan_load_accept_busy_hint_and_accepted_state(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let(w,store)=cx.update(|cx|{
        gpui_kit::init(cx);
        let session=serde_json::from_value(serde_json::json!({"id":"s","workspaceId":"w","workspaceName":"W","messages":[{"id":"p","role":"plan","content":"/plan.md","timestamp":1}],"lastMessageAt":1,"isProcessing":false})).unwrap();
        let store=cx.new(|_|{let mut s=SessionStore::new(session);s.model.set_draft("keep new draft".into());s.plan.content.insert("p".into(),Ok("[docs](https://example.com)\n\n# Plan\n\nRead and test.".into()));s});
        let options=WindowOptions{window_bounds:Some(WindowBounds::Windowed(Bounds::new(Point::default(),size(px(960.),px(600.))))),..Default::default()};
        let(w,v)=gpui_kit::open_window(options,cx,|_,cx|cx.new(|cx|PlanCard::new(store.clone(),"p".into(),cx))).unwrap();
        let sink=events.clone();cx.subscribe(&v,move|_,e:&ChatIntent,_|sink.borrow_mut().push(e.clone())).detach();(w,store)
    });
    cx.run_until_parked();
    assert!(
        events
            .borrow()
            .iter()
            .any(|e| matches!(e,ChatIntent::LoadPlan(id)if id=="p"))
    );
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("plan-draft-hint:p").is_some());
        w.click_at("markdown-document", gpui_kit::point(px(10.), px(10.)), cx);
        w.click("plan-accept:p", cx);
        w.click("plan-compact:p", cx);
        store.update(cx, |s, cx| {
            s.plan.pending = Some(1);
            cx.notify();
        });
        w.render_frame(cx);
        w.click("plan-accept:p", cx);
    })
    .unwrap();
    assert!(
        events
            .borrow()
            .iter()
            .any(|e| matches!(e,ChatIntent::LinkClicked(url)if url=="https://example.com"))
    );
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|e| matches!(e, ChatIntent::AcceptPlan { .. }))
            .count(),
        2
    );
    cx.update_window(w, |_, w, cx| {
        store.update(cx, |s, cx| {
            s.plan.pending = None;
            s.plan.accepted.insert("p".into());
            cx.notify();
        });
        w.render_frame(cx);
        assert!(w.try_find("plan-accepted:p").is_some());
        assert!(w.try_find("plan-accept:p").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn historical_plans_have_no_actions_and_read_failures_are_retryable(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let(w,store)=cx.update(|cx|{
        gpui_kit::init(cx);
        let session=serde_json::from_value(serde_json::json!({"id":"s","workspaceId":"w","workspaceName":"W","messages":[{"id":"old","role":"plan","content":"/old.md","timestamp":1},{"id":"new","role":"plan","content":"/new.md","timestamp":2}],"lastMessageAt":2,"isProcessing":false})).unwrap();
        let store=cx.new(|_|{let mut s=SessionStore::new(session);s.plan.content.insert("old".into(),Err(()));s});
        let options=WindowOptions{window_bounds:Some(WindowBounds::Windowed(Bounds::new(Point::default(),size(px(960.),px(600.))))),..Default::default()};
        let(w,v)=gpui_kit::open_window(options,cx,|_,cx|cx.new(|cx|PlanCard::new(store.clone(),"old".into(),cx))).unwrap();let sink=events.clone();cx.subscribe(&v,move|_,e:&ChatIntent,_|sink.borrow_mut().push(e.clone())).detach();(w,store)
    });
    cx.run_until_parked();
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("plan-accept:old").is_none());
        w.click("plan-retry:old", cx);
        store.update(cx, |s, cx| {
            s.model.state.session.messages.pop();
            cx.notify();
        });
        w.render_frame(cx);
        w.click("plan-accept:old", cx);
    })
    .unwrap();
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|e| matches!(e, ChatIntent::LoadPlan(_)))
            .count(),
        2
    );
    assert!(
        !events
            .borrow()
            .iter()
            .any(|e| matches!(e, ChatIntent::AcceptPlan { .. }))
    );
}
