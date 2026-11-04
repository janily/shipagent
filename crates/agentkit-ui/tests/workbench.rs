#![allow(clippy::unwrap_used)]
use agentkit_protocol::generated as g;
use agentkit_store::{
    core::{reducer::Context as Clock, session::SessionStoreEvent},
    entities::SessionStore,
};
use agentkit_ui::{WorkbenchIntent, WorkbenchView};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, Point, TestAppContext, WindowBounds,
    WindowOptions, px, size, test::TestWindowExt,
};
use std::{cell::RefCell, rc::Rc};
fn clock(id: &str) -> Clock {
    Clock {
        now_ms: 1700000000000,
        message_id: id.into(),
    }
}
fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<WorkbenchView>, Entity<SessionStore>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        let session:g::Session=serde_json::from_value(serde_json::json!({"id":"s","workspaceId":"w","workspaceName":"W","messages":[],"lastMessageAt":0,"isProcessing":false})).unwrap();
        let store=cx.new(|_|SessionStore::new(session));
        let (window,view)=gpui_kit::open_window(WindowOptions{window_bounds:Some(WindowBounds::Windowed(Bounds{origin:Point::default(),size:size(px(1280.),px(820.))})),..WindowOptions::default()},cx,|window,cx|cx.new(|cx|WorkbenchView::new("Workbench".into(),window,cx))).unwrap();
        window.update(cx,|_,window,cx|view.update(cx,|v,cx|{v.connected("W".into(),Some("model".into()),cx);v.bind_session(store.clone(),window,cx);})).unwrap();
        (window,view,store)
    })
}
fn apply(store: &Entity<SessionStore>, values: Vec<serde_json::Value>, cx: &mut gpui_kit::App) {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let batch: Vec<_> = values
        .into_iter()
        .map(|v| {
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            (
                serde_json::from_value(v).unwrap(),
                Clock {
                    now_ms: 1700000000000 + n as i64,
                    message_id: format!("m{n}"),
                },
            )
        })
        .collect();
    store.update(cx, |s, cx| {
        s.apply_batch(&batch, cx);
    });
}
#[gpui_kit::test]
fn composer_emits_submit_and_stop_using_native_controls(cx: &mut TestAppContext) {
    let (w, v, s) = open(cx);
    let sent = Rc::new(RefCell::new(Vec::new()));
    cx.update(|cx| {
        let sent = sent.clone();
        cx.subscribe(&v, move |_, e: &WorkbenchIntent, _| match e {
            WorkbenchIntent::Submit(t) => sent.borrow_mut().push(t.clone()),
            WorkbenchIntent::Cancel => sent.borrow_mut().push("STOP".into()),
            _ => {}
        })
        .detach();
    });
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("中文 test", cx);
        window.click("send", cx);
        s.update(cx, |s, cx| {
            s.model
                .begin_send("中文 test".into(), vec![], &clock("u"))
                .unwrap();
            cx.notify();
        });
        window.render_frame(cx);
        window.click("stop", cx);
    })
    .unwrap();
    assert_eq!(*sent.borrow(), ["中文 test", "STOP"]);
}
#[gpui_kit::test]
fn streamed_text_tool_result_and_final_answer_reach_the_window(cx: &mut TestAppContext) {
    let (w, _, s) = open(cx);
    cx.update(|cx|apply(&s,vec![serde_json::json!({"type":"text_delta","sessionId":"s","turnId":"t","delta":"流式"}),serde_json::json!({"type":"text_delta","sessionId":"s","turnId":"t","delta":"答案"})],cx));
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find(("message", 0usize)).label(), Some("流式答案"));
    })
    .unwrap();
    cx.update(|cx|apply(&s,vec![serde_json::json!({"type":"tool_start","sessionId":"s","toolUseId":"read","toolName":"Read","toolInput":{}}),serde_json::json!({"type":"tool_result","sessionId":"s","toolUseId":"read","toolName":"Read","result":"REAL_TOOL_RESULT"}),serde_json::json!({"type":"text_complete","sessionId":"s","turnId":"t","messageId":"final","text":"最终答案"}),serde_json::json!({"type":"complete","sessionId":"s"})],cx));
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find(("message", 0usize)).label(), Some("最终答案"));
        assert!(window.try_find("stop").is_none());
    })
    .unwrap();
}
#[gpui_kit::test]
fn a_failed_send_keeps_the_draft_available_for_retry(cx: &mut TestAppContext) {
    let (w, v, _) = open(cx);
    let sent = Rc::new(RefCell::new(Vec::new()));
    cx.update(|cx| {
        let sent = sent.clone();
        cx.subscribe(&v, move |_, e: &WorkbenchIntent, _| {
            if let WorkbenchIntent::Submit(t) = e {
                sent.borrow_mut().push(t.clone());
            }
        })
        .detach();
    });
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("preserve", cx);
        window.click("send", cx);
        v.update(cx, |v, cx| {
            v.submission_finished("preserve", false, window, cx);
            v.show_error("offline".into(), false, cx);
        });
        window.render_frame(cx);
        window.click("send", cx);
    })
    .unwrap();
    assert_eq!(*sent.borrow(), ["preserve", "preserve"]);
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("session-error").is_none());
    })
    .unwrap();
}
#[gpui_kit::test]
fn permission_buttons_emit_one_explicit_decision(cx: &mut TestAppContext) {
    let (w, v, s) = open(cx);
    let sent = Rc::new(RefCell::new(Vec::new()));
    cx.update(|cx| {
        let sent = sent.clone();
        cx.subscribe(&v, move |_, e: &WorkbenchIntent, _| {
            if let WorkbenchIntent::Permission {
                request_id,
                allowed,
                always,
            } = e
            {
                sent.borrow_mut()
                    .push((request_id.clone(), *allowed, *always));
            }
        })
        .detach();
    });
    cx.update_window(w,|_,window,cx|{apply(&s,vec![serde_json::json!({"type":"permission_request","sessionId":"s","request":{"sessionId":"s","requestId":"r","toolName":"Bash","description":"Allow?","canAlwaysAllow":true}})],cx);window.render_frame(cx);window.click("permission-deny",cx);window.click("permission-deny",cx);}).unwrap();
    assert_eq!(*sent.borrow(), [("r".into(), false, false)]);
}
#[gpui_kit::test]
fn loading_and_stale_recovery_preserve_the_draft_and_block_send(cx: &mut TestAppContext) {
    let (w, v, _) = open(cx);
    let sent = Rc::new(RefCell::new(Vec::new()));
    cx.update(|cx| {
        let sent = sent.clone();
        cx.subscribe(&v, move |_, e: &WorkbenchIntent, _| {
            if let WorkbenchIntent::Submit(t) = e {
                sent.borrow_mut().push(t.clone());
            }
        })
        .detach();
    });
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("keep", cx);
        v.update(cx, |v, cx| v.set_loading(cx));
        window.render_frame(cx);
        window.press("enter", cx);
        v.update(cx, |v, cx| {
            v.show_error("offline".into(), true, cx);
            v.transport_recovered(true, cx);
            v.transport_recovered(false, cx);
        });
        window.render_frame(cx);
        window.click("send", cx);
        assert!(sent.borrow().is_empty());
        v.update(cx, |v, cx| {
            v.connected("W".into(), Some("model".into()), cx)
        });
        window.render_frame(cx);
        window.click("send", cx);
    })
    .unwrap();
    assert_eq!(*sent.borrow(), ["keep"]);
}
#[gpui_kit::test]
fn entity_batches_combine_deltas_and_notify_once(cx: &mut TestAppContext) {
    let (_, _, s) = open(cx);
    let changes = Rc::new(RefCell::new((0, Vec::new())));
    cx.update(|cx| {
        let a = changes.clone();
        cx.observe(&s, move |_, _| a.borrow_mut().0 += 1).detach();
        let a = changes.clone();
        cx.subscribe(&s, move |_, event: &SessionStoreEvent, _| {
            a.borrow_mut().1.push(event.clone())
        })
        .detach();
        apply(
            &s,
            (0..100)
                .map(|_| serde_json::json!({"type":"text_delta","sessionId":"s","delta":"中"}))
                .collect(),
            cx,
        );
    });
    assert_eq!(changes.borrow().0, 1);
    assert_eq!(changes.borrow().1.len(), 1);
    assert!(
        matches!(&changes.borrow().1[0],SessionStoreEvent::StreamDelta{delta,..} if delta==&"中".repeat(100))
    );
}
#[gpui_kit::test]
fn drafts_survive_entity_switches(cx: &mut TestAppContext) {
    let (w, v, a) = open(cx);
    let b = cx.update(|cx| {
        let mut session = a.read(cx).model.state.session.clone();
        session.id = "b".into();
        cx.new(|_| SessionStore::new(session))
    });
    cx.update(|cx| {
        let a = a.clone();
        let b = b.clone();
        cx.subscribe(&v, move |_, intent: &WorkbenchIntent, cx| {
            if let WorkbenchIntent::DraftChanged { session_id, text } = intent {
                let store = if session_id == "s" { &a } else { &b };
                store.update(cx, |s, _| s.model.draft.text = text.clone());
            }
        })
        .detach();
    });
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("draft A", cx);
    })
    .unwrap();
    cx.update_window(w, |_, window, cx| {
        v.update(cx, |v, cx| v.bind_session(b.clone(), window, cx));
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("draft B", cx);
    })
    .unwrap();
    cx.update_window(w, |_, window, cx| {
        v.update(cx, |v, cx| v.bind_session(a.clone(), window, cx));
        assert_eq!(a.read(cx).model.draft.text, "draft A");
        assert_eq!(b.read(cx).model.draft.text, "draft B");
        assert_eq!(v.read(cx).current_draft(cx).unwrap().1, "draft A");
    })
    .unwrap();
}

#[cfg(feature = "samples")]
#[gpui_kit::test]
fn mixed_structural_and_stream_batches_never_double_append_markdown(cx: &mut TestAppContext) {
    let (w, v, store) = open(cx);
    cx.update(|cx|{
        let batch=vec![serde_json::json!({"type":"text_delta","sessionId":"s","turnId":"t","delta":"A"}),serde_json::json!({"type":"text_complete","sessionId":"s","turnId":"t","messageId":"authoritative","text":"A"}),serde_json::json!({"type":"text_delta","sessionId":"s","turnId":"next","delta":"B"})];
        let batch:Vec<_>=batch.into_iter().enumerate().map(|(i,v)|(serde_json::from_value(v).unwrap(),Clock{now_ms:1000+i as i64,message_id:format!("m{i}")})).collect();
        store.update(cx,|s,cx|s.apply_batch(&batch,cx));
    });
    cx.update(|cx|{
        assert_eq!(v.read(cx).cached_markdown("authoritative",cx).unwrap(),"A");assert_eq!(v.read(cx).cached_markdown("m2",cx).unwrap(),"B");
        let batch:Vec<_>=["C","D"].into_iter().map(|delta|(serde_json::from_value(serde_json::json!({"type":"text_delta","sessionId":"s","turnId":"next","delta":delta})).unwrap(),Clock{now_ms:1003,message_id:"unused".into()})).collect();store.update(cx,|s,cx|s.apply_batch(&batch,cx));
    });
    cx.update(|cx| {
        assert_eq!(v.read(cx).cached_markdown("m2", cx).unwrap(), "BCD");
    });
    cx.update_window(w, |_, _, _| {}).unwrap();
}
#[gpui_kit::test]
fn a_background_send_failure_is_visible_when_reopening_its_session(cx: &mut TestAppContext) {
    let (w, v, a) = open(cx);
    let b = cx.update(|cx| {
        let mut session = a.read(cx).model.state.session.clone();
        session.id = "b".into();
        cx.new(|_| SessionStore::new(session))
    });
    cx.update_window(w, |_, window, cx| {
        a.update(cx, |s, _| {
            s.model
                .begin_send("failed A".into(), vec![], &clock("failed"))
                .unwrap();
        });
        v.update(cx, |v, cx| v.bind_session(b.clone(), window, cx));
        a.update(cx, |s, _| {
            s.model.finish_send("failed", false);
            s.model.last_error = Some("A send rejected".into());
        });
        v.update(cx, |v, cx| v.bind_session(a.clone(), window, cx));
        window.render_frame(cx);
        assert_eq!(
            window.find("session-error").label(),
            Some("A send rejected")
        );
        assert_eq!(a.read(cx).model.draft.text, "failed A");
    })
    .unwrap();
}
#[gpui_kit::test]
fn invalid_semantic_events_preserve_valid_neighbors_and_report_an_effect(cx: &mut TestAppContext) {
    let (_, _, store) = open(cx);
    cx.update(|cx|{
        let batch:Vec<_>=[serde_json::json!({"type":"text_delta","sessionId":"s","delta":"A"}),serde_json::json!({"type":"text_complete","sessionId":"s","text":"bad","timestamp":0.5}),serde_json::json!({"type":"text_delta","sessionId":"s","delta":"B"})].into_iter().enumerate().map(|(i,v)|(serde_json::from_value(v).unwrap(),clock(&format!("err{i}")))).collect();
        let effects=store.update(cx,|s,cx|s.apply_batch(&batch,cx));
        assert_eq!(effects.len(),1);assert!(matches!(&effects[0],agentkit_store::core::reducer::Effect::ToastError{..}));assert_eq!(store.read(cx).model.state.session.messages[0].content,"AB");
    });
}
#[gpui_kit::test]
fn responding_to_first_permission_enables_the_next_request(cx: &mut TestAppContext) {
    let (w, v, s) = open(cx);
    let ids = Rc::new(RefCell::new(Vec::new()));
    cx.update(|cx|{let ids=ids.clone();cx.subscribe(&v,move|_,event:&WorkbenchIntent,_|{if let WorkbenchIntent::Permission{request_id,..}=event{ids.borrow_mut().push(request_id.clone());}}).detach();apply(&s,vec![serde_json::json!({"type":"permission_request","sessionId":"s","request":{"sessionId":"s","requestId":"first","toolName":"Bash","description":"first"}}),serde_json::json!({"type":"permission_request","sessionId":"s","request":{"sessionId":"s","requestId":"second","toolName":"Bash","description":"second"}})],cx);});
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click("permission-deny", cx);
    })
    .unwrap();
    cx.update(|cx| {
        s.update(cx, |s, cx| {
            s.model.permission_responded("first");
            cx.notify();
        })
    });
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        window.click("permission-deny", cx);
    })
    .unwrap();
    assert_eq!(*ids.borrow(), ["first", "second"]);
}
#[cfg(feature = "samples")]
#[gpui_kit::test]
fn removing_a_transient_tail_preserves_the_readers_scroll_anchor(cx: &mut TestAppContext) {
    let (w, v, original) = open(cx);
    let store=cx.update(|cx|{
        let mut session=original.read(cx).model.state.session.clone();session.is_processing=true;
        for i in 0..200 {session.messages.push(serde_json::from_value(serde_json::json!({"id":format!("history{i}"),"role":"assistant","content":"history content ".repeat(50),"timestamp":i})).unwrap());}
        session.messages.push(serde_json::from_value(serde_json::json!({"id":"status","role":"status","content":"working","timestamp":200})).unwrap());cx.new(|_|SessionStore::new(session))
    });
    cx.update_window(w, |_, window, cx| {
        v.update(cx, |v, cx| v.bind_session(store.clone(), window, cx));
        window.render_frame(cx);
        let scroll = v.read(cx).scroll_state().clone();
        scroll.update(cx, |s, cx| {
            s.scroll_to_item(0, cx);
        });
        window.render_frame(cx);
        assert!(!scroll.read(cx).is_following_tail());
    })
    .unwrap();
    cx.update(|cx| {
        apply(
            &store,
            vec![serde_json::json!({"type":"interrupted","sessionId":"s"})],
            cx,
        )
    });
    cx.update_window(w, |_, window, cx| {
        window.render_frame(cx);
        let scroll = v.read(cx).scroll_state();
        assert_eq!(scroll.read(cx).item_count(), 200);
        assert!(!scroll.read(cx).is_following_tail());
    })
    .unwrap();
}
