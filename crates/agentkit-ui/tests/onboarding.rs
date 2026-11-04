#![cfg(feature = "samples")]
#![allow(clippy::unwrap_used)]
use agentkit_ui::{
    onboarding::{OnboardingIntent, OnboardingView, resolve_slug},
    settings::{ConnectionEditor, ConnectionKind, EditorIntent},
};
use gpui_kit::{
    AppContext, Bounds, Point, TestAppContext, WindowBounds, WindowOptions, px, size,
    test::TestWindowExt,
};
use std::{cell::RefCell, rc::Rc};
#[::core::prelude::v1::test]
fn upstream_slug_rule_avoids_existing_suffixes() {
    assert_eq!(resolve_slug([]), "pi-api-key");
    assert_eq!(
        resolve_slug([
            "pi-api-key".into(),
            "pi-api-key-2".into(),
            "pi-api-key-4".into()
        ]),
        "pi-api-key-3"
    );
}
#[gpui_kit::test]
fn welcome_methods_deferral_and_oauth_are_explicit(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let (w, v) = cx.update(|cx| {
        gpui_kit::init(cx);
        let (w, v) =
            gpui_kit::open_window(options(), cx, |_, cx| cx.new(OnboardingView::new)).unwrap();
        let e = events.clone();
        cx.subscribe(&v, move |_, intent: &OnboardingIntent, _| {
            e.borrow_mut().push(intent.clone())
        })
        .detach();
        (w, v)
    });
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("onboarding-defer").is_none());
        assert!(
            w.find("onboarding-start").visible(),
            "start {:?}, root {:?}",
            w.find("onboarding-start").bounds(),
            w.find("onboarding").bounds()
        );
        w.click("onboarding-start", cx);
        w.render_frame(cx);
        w.click(("onboarding-method", 4usize), cx);
        w.click(("onboarding-method", 5usize), cx);
        w.click("onboarding-defer", cx);
        w.click("onboarding-defer", cx);
    })
    .unwrap();
    assert_eq!(events.borrow().len(), 1);
    assert!(matches!(&events.borrow()[0], OnboardingIntent::Defer));
    cx.update(|cx| {
        v.update(cx, |v, cx| {
            v.status(false, Some("backend rejected".into()), cx)
        })
    });
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click(("onboarding-method", 3usize), cx);
    })
    .unwrap();
    assert!(matches!(
        events.borrow().last(),
        Some(OnboardingIntent::Choose(ConnectionKind::Ollama))
    ));
}
#[gpui_kit::test]
fn editing_after_a_successful_test_requires_testing_again(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let (w, v) = cx.update(|cx| {
        gpui_kit::init(cx);
        let (w, v) = gpui_kit::open_window(options(), cx, |w, cx| {
            cx.new(|cx| ConnectionEditor::new("pi-api-key".into(), None, vec![], w, cx))
        })
        .unwrap();
        cx.update_window(w, |_, w, cx| {
            v.update(cx, |v, cx| {
                v.prepare_onboarding(ConnectionKind::Ollama, w, cx)
            })
        })
        .unwrap();
        let e = events.clone();
        cx.subscribe(&v, move |_, intent: &EditorIntent, _| {
            e.borrow_mut().push(intent.clone())
        })
        .detach();
        (w, v)
    });
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click("connection-model", cx);
        w.input("model", cx);
    })
    .unwrap();
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click("connection-editor-save", cx);
        w.click("connection-editor-test", cx);
    })
    .unwrap();
    assert_eq!(events.borrow().len(), 1);
    assert!(matches!(&events.borrow()[0], EditorIntent::Test(_)));
    cx.update(|cx| v.update(cx, |v, cx| v.test_passed(cx)));
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click("connection-model", cx);
        w.input("changed", cx);
    })
    .unwrap();
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click("connection-editor-save", cx);
    })
    .unwrap();
    assert_eq!(events.borrow().len(), 1);
    cx.update(|cx| v.update(cx, |v, cx| v.test_passed(cx)));
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click("connection-editor-save", cx);
    })
    .unwrap();
    assert!(matches!(
        events.borrow().last(),
        Some(EditorIntent::Save(_))
    ));
}

fn options() -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: Point::default(),
            size: size(px(1280.), px(900.)),
        })),
        ..Default::default()
    }
}
