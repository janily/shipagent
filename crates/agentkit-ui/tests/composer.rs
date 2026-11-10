#![cfg(feature = "samples")]
#![allow(clippy::unwrap_used)]
use agentkit_protocol::generated as g;
use agentkit_store::entities::{AppState, SessionStore, WorkspaceStore};
use agentkit_ui::{
    WorkbenchIntent, WorkbenchView,
    composer::{SendKey, model_choices},
};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, ElementInputHandler, Entity, EntityInputHandler,
    Focusable, Point, TestAppContext, WindowBounds, WindowOptions, px, size,
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
            v.set_connection_state(agentkit_ui::recovery::Phase::Online, false, cx);
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

#[gpui_kit::test]
fn attachment_only_send_paste_text_and_busy_import_gate(cx: &mut TestAppContext) {
    let (w, v, s, e) = open(cx);
    step(cx, w, |window, cx| {
        s.update(cx,|s,cx|{s.attachments.files.push(agentkit_store::core::attachments::Entry{id:"local".into(),file:std::sync::Arc::new(serde_json::from_value(serde_json::json!({"path":"/note.txt","name":"note.txt","type":"text","mimeType":"text/plain","size":4,"text":"note"})).unwrap())});s.attachments.reading=1;cx.notify();});
        window.render_frame(cx);
        window.click("send", cx);
    });
    assert!(sent(&e).is_empty());
    step(cx, w, |window, cx| {
        s.update(cx, |s, cx| {
            s.attachments.reading = 0;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("send", cx);
    });
    assert_eq!(sent(&e), [""]);
    step(cx, w, |window, cx| {
        v.update(cx, |v, cx| v.submission_finished("", false, window, cx));
        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string("ordinary text".into()));
        window.render_frame(cx);
        window.click("composer", cx);
        window.dispatch_action(Box::new(gpui_kit::component::input::Paste), cx);
        window.click("attach-files", cx);
    });
    cx.run_until_parked();
    step(cx, w, |_, cx| {
        assert_eq!(v.read(cx).current_draft(cx).unwrap().1, "ordinary text")
    });
    assert!(cx.did_prompt_for_paths());
    let other = cx.update(|cx| {
        let mut session = s.read(cx).model.state.session.clone();
        session.id = "other".into();
        cx.new(|_| SessionStore::new(session))
    });
    step(cx, w, |window, cx| {
        v.update(cx, |v, cx| v.bind_session(other, window, cx))
    });
    cx.simulate_path_prompt_response(|options| {
        assert!(options.multiple && options.files && !options.directories);
        Some(vec!["/first.txt".into(), "/second.txt".into()])
    });
    cx.run_until_parked();
    assert!(e.borrow().iter().any(|e|matches!(e,WorkbenchIntent::Attach{session_id,inputs}if session_id=="s"&&inputs.len()==2)));
}

#[gpui_kit::test]
fn native_attachment_remove_control_emits_stable_identity(cx: &mut TestAppContext) {
    let (w, _, s, e) = open(cx);
    step(cx, w, |window, cx| {
        s.update(cx,|s,cx|{s.attachments.files.push(agentkit_store::core::attachments::Entry{id:"remove-me".into(),file:std::sync::Arc::new(serde_json::from_value(serde_json::json!({"path":"/note.txt","name":"note.txt","type":"text","mimeType":"text/plain","size":4})).unwrap())});cx.notify();});
        window.render_frame(cx);
        window.hover("attachment-row:remove-me", cx);
        window.render_frame(cx);
        window.click(
            (
                gpui_kit::ElementId::from(gpui_kit::SharedString::from("attachment:remove-me")),
                "remove",
            ),
            cx,
        );
    });
    assert!(e.borrow().iter().any(|e|matches!(e,WorkbenchIntent::RemoveAttachment{session_id,id}if session_id=="s"&&id=="remove-me")));
}

#[gpui_kit::test]
fn image_clipboard_and_whole_chat_file_drop_emit_import_intents(cx: &mut TestAppContext) {
    let (w, _, _, e) = open(cx);
    step(cx, w, |window, cx| {
        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_image(
            &gpui_kit::Image::from_bytes(gpui_kit::ImageFormat::Png, vec![1, 2, 3]),
        ));
        window.render_frame(cx);
        window.click("composer", cx);
        window.dispatch_action(Box::new(gpui_kit::component::input::Paste), cx);
    });
    cx.run_until_parked();
    assert!(e.borrow().iter().any(|e|matches!(e,WorkbenchIntent::Attach{session_id,inputs}if session_id=="s" && matches!(&inputs[0],agentkit_ui::AttachmentInput::Image{name,bytes}if name=="pasted.png"&&bytes.as_ref()==[1,2,3]))));
    step(cx, w, |window, cx| {
        window.render_frame(cx);
        let position = window.find("composer").bounds().center();
        window.dispatch_event(
            gpui_kit::PlatformInput::FileDrop(gpui_kit::FileDropEvent::Entered {
                position,
                paths: gpui_kit::ExternalPaths(vec![std::path::PathBuf::from("/drop.txt")].into()),
            }),
            cx,
        );
        window.render_frame(cx);
        window.dispatch_event(
            gpui_kit::PlatformInput::FileDrop(gpui_kit::FileDropEvent::Submit { position }),
            cx,
        );
    });
    cx.run_until_parked();
    assert!(e.borrow().iter().any(|e|matches!(e,WorkbenchIntent::Attach{session_id,inputs}if session_id=="s" && matches!(&inputs[0],agentkit_ui::AttachmentInput::Path(path)if path==std::path::Path::new("/drop.txt")))));
}

#[gpui_kit::test]
fn slash_menu_uses_cached_skill_and_atomic_token_with_undo(cx: &mut TestAppContext) {
    let (w, v, _, e) = open(cx);
    step(cx, w, |window, cx| {
        let app = cx.new(|_| AppState {
            connections: vec![connection("valid", true)],
            ..Default::default()
        });
        let workspace=cx.new(|_|WorkspaceStore{workspace:Some(serde_json::from_value(serde_json::json!({"id":"w","name":"W","slug":"my workspace","rootPath":"/tmp/w","createdAt":0})).unwrap()),skills:vec![serde_json::from_value(serde_json::json!({"slug":"review-code","path":"/tmp/skill","content":"instructions","source":"workspace","metadata":{"name":"Review","description":"Review changes"}})).unwrap()],..Default::default()});
        v.update(cx, |v, cx| v.bind_context(app, workspace, cx));
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("/rev", cx);
        window.render_frame(cx);
        window.press("tab", cx);
    });
    cx.run_until_parked();
    step(cx, w, |window, cx| {
        let input = v.read(cx).composer_input(cx);
        assert_eq!(input.read(cx).value(), "[skill:my workspace:review-code]");
        assert_eq!(input.read(cx).tokens().len(), 1);
        assert!(input.read(cx).focus_handle(cx).is_focused(window));
        window.press("backspace", cx);
        assert_eq!(input.read(cx).value(), "");
        window.press(
            if cfg!(target_os = "macos") {
                "cmd-z"
            } else {
                "ctrl-z"
            },
            cx,
        );
    });
    cx.run_until_parked();
    step(cx, w, |window, cx| {
        let input = v.read(cx).composer_input(cx);
        assert_eq!(input.read(cx).value(), "[skill:my workspace:review-code]");
        assert_eq!(input.read(cx).tokens().len(), 1);
        window.press("enter", cx);
    });
    assert_eq!(sent(&e), ["[skill:my workspace:review-code]"]);
}
#[gpui_kit::test]
fn file_search_is_debounced_and_stale_results_cannot_replace_current_query(
    cx: &mut TestAppContext,
) {
    let (w, v, _, e) = open(cx);
    step(cx, w, |window, cx| {
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("@old", cx);
    });
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(160));
    cx.run_until_parked();
    let (request, query) = e
        .borrow()
        .iter()
        .rev()
        .find_map(|e| {
            if let WorkbenchIntent::SearchMentions { request, query, .. } = e {
                Some((*request, query.clone()))
            } else {
                None
            }
        })
        .unwrap();
    step(cx, w, |window, cx| {
        window.input("er", cx);
        v.update(cx,|v,cx|v.mention_results("s",request,&query,Ok(vec![serde_json::from_value(serde_json::json!({"name":"wrong","path":"/tmp/wrong","relativePath":"wrong","type":"file"})).unwrap()]),window,cx));
        window.press("enter", cx);
    });
    assert!(sent(&e).is_empty());
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(160));
    cx.run_until_parked();
    let request = e
        .borrow()
        .iter()
        .rev()
        .find_map(|e| {
            if let WorkbenchIntent::SearchMentions { request, query, .. } = e {
                assert_eq!(query, "older");
                Some(*request)
            } else {
                None
            }
        })
        .unwrap();
    step(cx, w, |window, cx| {
        v.update(cx,|v,cx|v.mention_results("s",request,"older",Ok(vec![serde_json::from_value(serde_json::json!({"name":"你好.rs","path":"/tmp/你好.rs","relativePath":"src/你好.rs","type":"file"})).unwrap()]),window,cx));
        window.render_frame(cx);
        window.press("enter", cx);
    });
    cx.run_until_parked();
    step(cx, w, |_, cx| {
        let input = v.read(cx).composer_input(cx);
        assert_eq!(input.read(cx).value(), "[file:src/你好.rs]");
        assert_eq!(input.read(cx).tokens().len(), 1);
    });
    assert!(sent(&e).is_empty());
}
#[gpui_kit::test]
fn escape_closes_menu_without_cancelling_and_ime_never_confirms(cx: &mut TestAppContext) {
    let (w, v, s, e) = open(cx);
    step(cx, w, |window, cx| {
        s.update(cx, |s, cx| {
            s.model.state.session.is_processing = true;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("/", cx);
        window.render_frame(cx);
        let input = v.read(cx).composer_input(cx);
        input.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, "compact", None, window, cx)
        });
        window.press("enter", cx);
        assert!(input.read(cx).tokens().is_empty());
        input.update(cx, |input, cx| input.unmark_text(window, cx));
        window.input("x", cx);
        window.press("escape", cx);
    });
    assert!(sent(&e).is_empty());
    assert!(
        !e.borrow()
            .iter()
            .any(|e| matches!(e, WorkbenchIntent::Cancel))
    );
}

#[gpui_kit::test]
fn compact_selection_submits_next_enter_and_escape_allows_literal_slash(cx: &mut TestAppContext) {
    let (w, v, _, e) = open(cx);
    step(cx, w, |window, cx| {
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("/comp", cx);
        window.press("enter", cx);
    });
    cx.run_until_parked();
    step(cx, w, |window, cx| {
        assert_eq!(v.read(cx).composer_input(cx).read(cx).tokens().len(), 1);
        window.press("enter", cx);
    });
    assert_eq!(sent(&e), ["/compact"]);
    step(cx, w, |window, cx| {
        v.update(cx, |v, cx| {
            v.submission_finished("/compact", true, window, cx)
        });
        window.input("/unknown", cx);
        window.press("escape", cx);
        window.press("enter", cx);
    });
    assert_eq!(sent(&e), ["/compact", "/unknown"]);
}
#[gpui_kit::test]
fn mention_retry_renews_request_and_switching_sessions_invalidates_search(cx: &mut TestAppContext) {
    let (w, v, _, e) = open(cx);
    step(cx, w, |window, cx| {
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("@src", cx);
    });
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(160));
    cx.run_until_parked();
    let request = e
        .borrow()
        .iter()
        .rev()
        .find_map(|e| {
            if let WorkbenchIntent::SearchMentions { request, .. } = e {
                Some(*request)
            } else {
                None
            }
        })
        .unwrap();
    step(cx, w, |window, cx| {
        v.update(cx, |v, cx| {
            v.mention_results(
                "s",
                request,
                "src",
                Err("fixture search error".into()),
                window,
                cx,
            )
        });
        window.press("enter", cx);
    });
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(160));
    cx.run_until_parked();
    let next = e
        .borrow()
        .iter()
        .rev()
        .find_map(|e| {
            if let WorkbenchIntent::SearchMentions { request, .. } = e {
                Some(*request)
            } else {
                None
            }
        })
        .unwrap();
    assert!(next > request);
    step(cx, w, |window, cx| {
        let session=serde_json::from_value(serde_json::json!({"id":"other","workspaceId":"w","workspaceName":"W","messages":[],"lastMessageAt":0,"isProcessing":false})).unwrap();
        let other = cx.new(|_| SessionStore::new(session));
        v.update(cx, |v, cx| v.bind_session(other, window, cx));
        v.update(cx,|v,cx|v.mention_results("s",next,"src",Ok(vec![serde_json::from_value(serde_json::json!({"name":"old","path":"/old","relativePath":"old","type":"file"})).unwrap()]),window,cx));
        assert_eq!(v.read(cx).composer_input(cx).read(cx).value(), "");
        assert!(v.read(cx).composer_input(cx).read(cx).tokens().is_empty());
    });
}
#[gpui_kit::test]
fn restored_file_brackets_are_atomic_and_group_arrows_select_folder(cx: &mut TestAppContext) {
    let (w, v, _, e) = open(cx);
    step(cx, w, |window, cx| {
        window.render_frame(cx);
        v.update(cx, |v, cx| {
            v.restore_draft("😀 [file:src/你好.rs]", window, cx)
        });
        assert_eq!(v.read(cx).composer_input(cx).read(cx).tokens().len(), 1);
        v.read(cx)
            .composer_input(cx)
            .update(cx, |i, cx| i.set_value("", window, cx));
        window.click("composer", cx);
        window.input("@src", cx);
    });
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(160));
    cx.run_until_parked();
    let request = e
        .borrow()
        .iter()
        .rev()
        .find_map(|e| {
            if let WorkbenchIntent::SearchMentions { request, .. } = e {
                Some(*request)
            } else {
                None
            }
        })
        .unwrap();
    step(cx, w, |window, cx| {
        let files=serde_json::from_value(serde_json::json!([{"name":"main.rs","path":"/p/src/main.rs","relativePath":"src/main.rs","type":"file"},{"name":"src","path":"/p/src","relativePath":"src","type":"directory"}])).unwrap();
        v.update(cx, |v, cx| {
            v.mention_results("s", request, "src", Ok(files), window, cx)
        });
        window.render_frame(cx);
        window.press("down", cx);
        window.press("tab", cx);
    });
    cx.run_until_parked();
    step(cx, w, |_, cx| {
        assert_eq!(
            v.read(cx).composer_input(cx).read(cx).value(),
            "[folder:src]"
        )
    });
}

#[gpui_kit::test]
fn history_recall_restores_atomic_file_and_directory_change_invalidates_results(
    cx: &mut TestAppContext,
) {
    let (w, v, s, e) = open(cx);
    step(cx, w, |window, cx| {
        s.update(cx,|s,cx|{s.model.state.session.messages.push(serde_json::from_value(serde_json::json!({"id":"u","role":"user","content":"[file:src/main.rs]","timestamp":0})).unwrap());cx.notify();});
        window.render_frame(cx);
        window.click("composer", cx);
        window.press("up", cx);
        assert_eq!(v.read(cx).composer_input(cx).read(cx).tokens().len(), 1);
        window.press("backspace", cx);
        assert_eq!(v.read(cx).composer_input(cx).read(cx).value(), "");
        window.input("@src", cx);
    });
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(160));
    cx.run_until_parked();
    let request = e
        .borrow()
        .iter()
        .rev()
        .find_map(|e| {
            if let WorkbenchIntent::SearchMentions { request, .. } = e {
                Some(*request)
            } else {
                None
            }
        })
        .unwrap();
    step(cx, w, |window, cx| {
        s.update(cx, |s, cx| {
            s.model.state.session.working_directory = Some("/other/directory".into());
            cx.notify();
        });
        window.render_frame(cx);
        v.update(cx,|v,cx|v.mention_results("s",request,"src",Ok(vec![serde_json::from_value(serde_json::json!({"name":"old","path":"/old","relativePath":"old","type":"file"})).unwrap()]),window,cx));
        window.press("enter", cx);
        assert!(v.read(cx).composer_input(cx).read(cx).tokens().is_empty());
    });
    assert!(sent(&e).is_empty());
}

#[gpui_kit::test]
fn mention_popover_is_visible_and_mouse_selection_keeps_editor_focus(cx: &mut TestAppContext) {
    let (w, v, _, e) = open(cx);
    step(cx, w, |window, cx| {
        window.render_frame(cx);
        window.click("composer", cx);
        window.input("/comp", cx);
    });
    cx.run_until_parked();
    step(cx, w, |window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("mention-menu").is_some());
        window
            .within("mention-menu")
            .click(("mention-option", 0usize), cx);
    });
    cx.run_until_parked();
    step(cx, w, |window, cx| {
        let input = v.read(cx).composer_input(cx);
        assert_eq!(input.read(cx).value(), "/compact");
        assert!(input.read(cx).focus_handle(cx).is_focused(window));
    });
    assert!(sent(&e).is_empty());
}
