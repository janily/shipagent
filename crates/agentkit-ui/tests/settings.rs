#![cfg(feature = "samples")]
#![allow(clippy::unwrap_used)]
use agentkit_protocol::generated as g;
use agentkit_store::entities::AppState;
use agentkit_ui::settings::{
    ConnectionEditor, ConnectionKind, EditorIntent, LocalChange, SettingsIntent, SettingsView,
};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, Point, TestAppContext, WindowBounds,
    WindowOptions, px, size, test::TestWindowExt,
};
use std::{cell::RefCell, rc::Rc};
fn connection() -> g::LlmConnectionWithStatus {
    serde_json::from_value(serde_json::json!({"slug":"local","name":"Local fixture","providerType":"pi_compat","authType":"none","baseUrl":"http://127.0.0.1:11434/v1","isAuthenticated":true,"isDefault":true,"defaultModel":"model","models":["model"],"createdAt":1})).unwrap()
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
type Harness = (
    AnyWindowHandle,
    Entity<SettingsView>,
    Rc<RefCell<Vec<SettingsIntent>>>,
);
fn open(cx: &mut TestAppContext) -> Harness {
    cx.update(|cx| {
        gpui_kit::init(cx);
        let app = cx.new(|_| AppState {
            connected: true,
            connections: vec![connection()],
            ..Default::default()
        });
        let (w, v) =
            gpui_kit::open_window(options(), cx, |_, cx| cx.new(SettingsView::new)).unwrap();
        v.update(cx, |s, cx| {
            s.bind(app, cx);
            s.defaults(g::ThinkingLevel::Medium, Some(200000), cx);
        });
        let e = Rc::new(RefCell::new(vec![]));
        let sink = e.clone();
        cx.subscribe(&v, move |_, e: &SettingsIntent, _| {
            sink.borrow_mut().push(e.clone())
        })
        .detach();
        (w, v, e)
    })
}
#[gpui_kit::test]
fn model_controls_use_stable_ids_and_disable_while_an_operation_is_pending(
    cx: &mut TestAppContext,
) {
    let (w, v, e) = open(cx);
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click("connection-test:local", cx);
        w.click("connection-default:local", cx);
    })
    .unwrap();
    assert_eq!(e.borrow().len(), 1);
    assert!(matches!(&e.borrow()[0],SettingsIntent::Test(id)if id=="local"));
    cx.update(|cx| v.update(cx, |s, cx| s.status(true, None, None, cx)));
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click("connection-delete:local", cx);
        w.click("connection-add", cx);
    })
    .unwrap();
    assert_eq!(e.borrow().len(), 1);
}
#[gpui_kit::test]
fn general_and_appearance_controls_emit_single_local_changes(cx: &mut TestAppContext) {
    let (w, _, e) = open(cx);
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.within("settings-sidebar").click("0-0", cx);
        w.render_frame(cx);
        w.click("settings-en", cx);
        w.click("settings-restore", cx);
        w.within("settings-sidebar").click("0-1", cx);
        w.render_frame(cx);
        w.click(("settings-theme", 2usize), cx);
        w.click(("settings-font", 16usize), cx);
        w.click("settings-fade", cx);
    })
    .unwrap();
    let e = e.borrow();
    assert!(
        e.iter()
            .any(|e| matches!(e,SettingsIntent::Local(LocalChange::Language(s))if s=="en"))
    );
    assert!(
        e.iter()
            .any(|e| matches!(e, SettingsIntent::Local(LocalChange::Font(16))))
    );
    assert!(
        e.iter()
            .any(|e| matches!(e, SettingsIntent::Local(LocalChange::Fade(false))))
    );
}
#[gpui_kit::test]
fn existing_key_is_never_loaded_and_requires_explicit_replacement(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::<EditorIntent>::new()));
    let (w, v) = cx.update(|cx| {
        gpui_kit::init(cx);
        let mut c = connection();
        c.auth_type = g::LlmConnectionWithStatusAuthType::ApiKeyWithEndpoint;
        let (w, v) = gpui_kit::open_window(options(), cx, |w, cx| {
            cx.new(|cx| ConnectionEditor::new("local".into(), Some(c), vec![], w, cx))
        })
        .unwrap();
        let sink = events.clone();
        cx.subscribe(&v, move |_, e: &EditorIntent, _| {
            sink.borrow_mut().push(e.clone())
        })
        .detach();
        (w, v)
    });
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("connection-key").is_none());
        w.click("connection-editor-save", cx);
    })
    .unwrap();
    assert!(
        matches!(&events.borrow()[0],EditorIntent::Save(v)if v.credential.is_none()&&v.existing)
    );
    cx.update(|cx| v.update(cx, |v, cx| v.status(false, None, None, cx)));
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click("connection-replace-key", cx);
        w.render_frame(cx);
        w.click("connection-key", cx);
        w.input("fixture-only-secret", cx);
        w.click("connection-editor-test", cx);
    })
    .unwrap();
    assert!(matches!(events.borrow().last(),Some(EditorIntent::Test(v))if v.credential.is_some()));
}
#[gpui_kit::test]
fn new_ollama_form_has_no_key_and_sends_reviewed_values_once(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::<EditorIntent>::new()));
    let (w, _) = cx.update(|cx| {
        gpui_kit::init(cx);
        let (w, v) = gpui_kit::open_window(options(), cx, |w, cx| {
            cx.new(|cx| ConnectionEditor::new("new-local".into(), None, vec![], w, cx))
        })
        .unwrap();
        let sink = events.clone();
        cx.subscribe(&v, move |_, e: &EditorIntent, _| {
            sink.borrow_mut().push(e.clone())
        })
        .detach();
        (w, v)
    });
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click(("connection-kind", 3usize), cx);
        w.render_frame(cx);
        assert!(w.try_find("connection-key").is_none());
        w.click("connection-name", cx);
        w.input("Local", cx);
        w.click("connection-model", cx);
        w.input("model", cx);
        w.click("connection-editor-save", cx);
        w.click("connection-editor-save", cx);
    })
    .unwrap();
    let e = events.borrow();
    assert_eq!(e.len(), 1);
    assert!(
        matches!(&e[0],EditorIntent::Save(v)if v.kind==ConnectionKind::Ollama&&v.credential.is_none()&&v.url=="http://127.0.0.1:11434/v1"&&v.models==["model"])
    );
}
#[gpui_kit::test]
fn keyless_anthropic_editor_preserves_the_saved_wire_protocol(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::<EditorIntent>::new()));
    let (w, _) = cx.update(|cx| {
        gpui_kit::init(cx);
        let mut c = connection();
        c.custom_endpoint = Some(g::CustomEndpointConfig {
            api: g::CustomEndpointConfigApi::AnthropicMessages,
            supports_images: Some(true),
            max_tokens: Some(4096.),
        });
        let (w, v) = gpui_kit::open_window(options(), cx, |w, cx| {
            cx.new(|cx| ConnectionEditor::new("local".into(), Some(c), vec![], w, cx))
        })
        .unwrap();
        let sink = events.clone();
        cx.subscribe(&v, move |_, e: &EditorIntent, _| {
            sink.borrow_mut().push(e.clone())
        })
        .detach();
        (w, v)
    });
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click("connection-editor-save", cx);
    })
    .unwrap();
    assert!(
        matches!(&events.borrow()[0],EditorIntent::Save(v)if v.kind==ConnectionKind::Anthropic&&v.existing)
    );
}
#[gpui_kit::test]
fn provider_response_cannot_fill_an_editor_after_switching_to_ollama(cx: &mut TestAppContext) {
    let (w, v) = cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit::open_window(options(), cx, |w, cx| {
            cx.new(|cx| {
                ConnectionEditor::new(
                    "local".into(),
                    None,
                    vec![("anthropic".into(), "Anthropic".into())],
                    w,
                    cx,
                )
            })
        })
        .unwrap()
    });
    let (provider, generation) = cx.update(|cx| v.read(cx).catalog_request().unwrap());
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click(("connection-kind", 3usize), cx);
        v.update(cx, |v, cx| {
            v.models(
                &provider,
                generation,
                vec![("old-preset".into(), "Old model".into())],
                w,
                cx,
            )
        });
        w.render_frame(cx);
        assert_eq!(
            w.within("connection-model").find("input-state").value(),
            Some("")
        );
        assert!(!v.read(cx).accepts_catalog(&provider, generation));
    })
    .unwrap();
}
#[gpui_kit::test]
fn changing_provider_clears_the_unsubmitted_credential(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::<EditorIntent>::new()));
    let (w, _) = cx.update(|cx| {
        gpui_kit::init(cx);
        let (w, v) = gpui_kit::open_window(options(), cx, |w, cx| {
            cx.new(|cx| {
                ConnectionEditor::new(
                    "new".into(),
                    None,
                    vec![
                        ("anthropic".into(), "Anthropic".into()),
                        ("openai".into(), "OpenAI".into()),
                    ],
                    w,
                    cx,
                )
            })
        })
        .unwrap();
        let sink = events.clone();
        cx.subscribe(&v, move |_, e: &EditorIntent, _| {
            sink.borrow_mut().push(e.clone())
        })
        .detach();
        (w, v)
    });
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click("connection-key", cx);
        w.input("fixture-key", cx);
        w.click("connection-provider", cx);
        w.within("popup-menu").click(1usize, cx);
    })
    .unwrap();
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(w.within("connection-key").find("input-state").value(), None);
        w.click("connection-name", cx);
        w.input("New", cx);
        w.click("connection-model", cx);
        w.input("model", cx);
        w.click("connection-editor-save", cx);
    })
    .unwrap();
    assert!(events.borrow().iter().any(
        |e| matches!(e,EditorIntent::Save(v) if v.provider=="openai"&&v.credential.is_none())
    ));
}

#[gpui_kit::test]
fn compaction_choices_match_backend_bounds_and_none_restores_sdk_default(cx: &mut TestAppContext) {
    let (w, _, events) = open(cx);
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        for i in 0usize..5 {
            w.click(("settings-compact", i), cx);
        }
    })
    .unwrap();
    let values: Vec<_> = events
        .borrow()
        .iter()
        .filter_map(|e| {
            if let SettingsIntent::Compact(tokens) = e {
                Some(*tokens)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        values,
        vec![
            None,
            Some(100000),
            Some(200000),
            Some(500000),
            Some(1000000)
        ]
    );
}
#[gpui_kit::test]
fn shortcuts_page_shows_effective_keys_and_reports_invalid_files(cx: &mut TestAppContext) {
    let (w, v, events) = open(cx);
    cx.update_window(w, |_, w, cx| {
        v.update(cx, |v, cx| {
            v.shortcuts(
                vec![(
                    "New conversation".into(),
                    "ctrl-k".into(),
                    "Workbench".into(),
                )],
                true,
                true,
                cx,
            )
        });
        w.render_frame(cx);
        w.render_frame(cx);
        assert!(w.try_find("shortcut-list").is_some());
        assert!(w.try_find("invalid-keymap").is_some());
        w.click("reload-keymap", cx);
        w.click("open-keymap", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(
        events
            .borrow()
            .iter()
            .any(|e| matches!(e, SettingsIntent::ReloadKeymap))
    );
    assert!(
        events
            .borrow()
            .iter()
            .any(|e| matches!(e, SettingsIntent::OpenKeymap))
    );
}

#[gpui_kit::test]
fn chatgpt_login_disables_duplicate_start_but_keeps_cancel_available(cx: &mut TestAppContext) {
    let (w, v, events) = open(cx);
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click("settings-chatgpt-login", cx);
    })
    .unwrap();
    assert!(matches!(
        events.borrow().as_slice(),
        [SettingsIntent::StartChatGpt]
    ));
    cx.update(|cx| {
        v.update(cx, |v, cx| {
            v.oauth_pending(true, cx);
            v.status(true, None, None, cx);
        })
    });
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click("settings-chatgpt-login", cx);
        w.click("settings-oauth-cancel", cx);
    })
    .unwrap();
    assert_eq!(events.borrow().len(), 2);
    assert!(matches!(events.borrow()[1], SettingsIntent::CancelChatGpt));
}
