#![cfg(feature = "samples")]
#![allow(clippy::unwrap_used)]
use agentkit_protocol::generated as g;
use agentkit_store::entities::{SessionListStore, WorkspaceStore};
use agentkit_ui::sidebar::{NavigationDialog, NavigationForm, SessionSidebar, SidebarIntent};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, Point, TestAppContext, WindowBounds,
    WindowOptions, px, size, test::TestWindowExt,
};
use std::{cell::RefCell, rc::Rc};
type Harness = (
    AnyWindowHandle,
    Entity<SessionSidebar>,
    Entity<SessionListStore>,
    Rc<RefCell<Vec<SidebarIntent>>>,
);
fn session(id: usize, time: i64) -> g::Session {
    serde_json::from_value(serde_json::json!({"id":format!("s{id}"),"name":format!("Session {id}"),"workspaceId":"w","workspaceName":"W","messages":[],"lastMessageAt":time,"isProcessing":false})).unwrap()
}
fn open(cx: &mut TestAppContext, n: usize) -> Harness {
    cx.update(|cx|{
        gpui_kit::init(cx);
        let list=cx.new(|_|SessionListStore::default());
        list.update(cx,|s,cx|s.replace((0..n).map(|i|session(i,i as i64)).collect(),cx));
        let workspace=cx.new(|_|WorkspaceStore{id:"w".into(),name:"Workspace".into(),catalog:serde_json::from_value(serde_json::json!([{"id":"w","name":"Workspace","slug":"w","rootPath":"/tmp/w","createdAt":0},{"id":"other","name":"Other","slug":"other","rootPath":"/tmp/other","createdAt":0}])).unwrap(),..Default::default()});
        let (w,v)=gpui_kit::open_window(WindowOptions{window_bounds:Some(WindowBounds::Windowed(Bounds{origin:Point::default(),size:size(px(280.),px(820.))})),..Default::default()},cx,|window,cx|cx.new(|cx|SessionSidebar::new(window,cx))).unwrap();
        w.update(cx,|_,_,cx|v.update(cx,|s,cx|{s.bind_list(list.clone(),cx);s.bind_workspace(workspace,cx);s.set_context(Some("s0".into()),true,cx);})).unwrap();
        let events=Rc::new(RefCell::new(vec![]));let sink=events.clone();cx.subscribe(&v,move|_,e:&SidebarIntent,_|sink.borrow_mut().push(e.clone())).detach();(w,v,list,events)
    })
}
#[gpui_kit::test]
fn thousand_rows_are_virtual_and_keyboard_navigation_uses_recent_order(cx: &mut TestAppContext) {
    let (w, _, _, events) = open(cx, 1000);
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("session:s999").is_some());
        assert!(w.try_find("session:s500").is_none());
        w.click("session:s999", cx);
        w.press("end", cx);
        w.press("enter", cx);
        w.press("home", cx);
        w.press("down", cx);
        w.press("f2", cx);
        w.press("delete", cx);
        w.render_frame(cx);
    })
    .unwrap();
    let e = events.borrow();
    assert!(matches!(&e[0],SidebarIntent::Open(id) if id=="s999"));
    assert!(matches!(&e[1],SidebarIntent::Open(id) if id=="s0"));
    assert!(matches!(&e[2],SidebarIntent::Rename(id) if id=="s998"));
    assert!(matches!(&e[3],SidebarIntent::Delete(id) if id=="s998"));
}
#[gpui_kit::test]
fn title_search_and_escape_keep_delete_from_leaking_out_of_input(cx: &mut TestAppContext) {
    let (w, _, _, events) = open(cx, 30);
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click("session-search", cx);
        w.input("Session 12", cx);
    })
    .unwrap();
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("session:s12").is_some());
        assert!(w.try_find("session:s29").is_none());
        w.press("backspace", cx);
        w.press("escape", cx);
    })
    .unwrap();
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("session:s29").is_some());
    })
    .unwrap();
    assert!(events.borrow().is_empty());
}
#[gpui_kit::test]
fn hidden_and_archived_are_excluded_and_badges_update_on_canonical_notification(
    cx: &mut TestAppContext,
) {
    let (w, _, list, _) = open(cx, 5);
    cx.update(|cx| {
        list.update(cx, |s, cx| {
            s.model.sessions[4].hidden = Some(true);
            s.model.sessions[3].is_archived = Some(true);
            s.model.awaiting.insert("s2".into(), vec!["r".into()]);
            cx.notify();
        })
    });
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("session:s4").is_none());
        assert!(w.try_find("session:s3").is_none());
        assert!(w.try_find("approval-dot:s2").is_some());
    })
    .unwrap();
}
#[gpui_kit::test]
fn disconnected_navigation_cannot_open_or_rename(cx: &mut TestAppContext) {
    let (w, v, _, events) = open(cx, 3);
    cx.update_window(w, |_, w, cx| w.render_frame(cx)).unwrap();
    cx.update(|cx| v.update(cx, |s, cx| s.set_context(None, false, cx)));
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click("session:s2", cx);
        w.press("f2", cx);
        w.click("new-session", cx);
    })
    .unwrap();
    assert!(events.borrow().is_empty());
}
#[gpui_kit::test]
fn rename_form_emits_once_and_failure_allows_retry_without_losing_text(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::<NavigationForm>::new()));
    let (w, v) = cx.update(|cx| {
        gpui_kit::init(cx);
        let (w, v) = gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| NavigationDialog::new("", false, window, cx))
        })
        .unwrap();
        let sink = events.clone();
        cx.subscribe(&v, move |_, f: &NavigationForm, _| {
            sink.borrow_mut().push(f.clone())
        })
        .detach();
        (w, v)
    });
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(
            w.find("navigation-name").label(),
            Some(agentkit_ui::t!("ak.nav.name").as_str())
        );
        w.click("navigation-name", cx);
        w.input("New title", cx);
        w.click("navigation-submit", cx);
        w.click("navigation-submit", cx);
        v.update(cx, |v, cx| v.status(false, Some("Retry".into()), cx));
        w.render_frame(cx);
        w.click("navigation-submit", cx);
    })
    .unwrap();
    let e = events.borrow();
    assert_eq!(e.len(), 2);
    assert_eq!(e[0].name, "New title");
    assert_eq!(e[1].name, "New title");
}
#[gpui_kit::test]
fn workspace_form_uses_directory_only_picker_and_retains_selection_on_cancel(
    cx: &mut TestAppContext,
) {
    let events = Rc::new(RefCell::new(Vec::<NavigationForm>::new()));
    let (w, _) = cx.update(|cx| {
        gpui_kit::init(cx);
        let (w, v) = gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| NavigationDialog::new("Project", true, window, cx))
        })
        .unwrap();
        let sink = events.clone();
        cx.subscribe(&v, move |_, f: &NavigationForm, _| {
            sink.borrow_mut().push(f.clone())
        })
        .detach();
        (w, v)
    });
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(
            w.find("navigation-directory").label(),
            Some(agentkit_ui::t!("ak.nav.directory").as_str())
        );
        w.click("navigation-pick", cx);
    })
    .unwrap();
    assert!(cx.did_prompt_for_paths());
    cx.simulate_path_prompt_response(|options| {
        assert!(options.directories);
        assert!(!options.files);
        assert!(!options.multiple);
        Some(vec![std::path::PathBuf::from("/tmp/project")])
    });
    cx.run_until_parked();
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click("navigation-pick", cx);
    })
    .unwrap();
    cx.simulate_path_prompt_response(|_| None);
    cx.run_until_parked();
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click("navigation-submit", cx);
    })
    .unwrap();
    let e = events.borrow();
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].directory, "/tmp/project");
    assert_eq!(e[0].name, "Project");
}
#[gpui_kit::test]
fn activity_reorders_rows_and_complete_applies_unread_metadata(cx: &mut TestAppContext) {
    let (w, _, list, events) = open(cx, 1000);
    let event=serde_json::from_value(serde_json::json!({"type":"user_message","sessionId":"s0","status":"accepted","message":{"id":"u","role":"user","content":"new activity","timestamp":99999}})).unwrap();
    cx.update(|cx| list.update(cx, |s, cx| s.apply_batch([&event], cx)));
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("session:s0").is_some());
        w.click("session:s0", cx);
        w.press("home", cx);
        w.press("enter", cx);
    })
    .unwrap();
    assert!(matches!(events.borrow().last(),Some(SidebarIntent::Open(id)) if id=="s0"));
    let event = serde_json::from_value(
        serde_json::json!({"type":"complete","sessionId":"s0","hasUnread":true}),
    )
    .unwrap();
    cx.update(|cx| list.update(cx, |s, cx| s.apply_batch([&event], cx)));
    cx.update(|cx| {
        assert_eq!(
            list.read(cx)
                .model
                .sessions
                .iter()
                .find(|s| s.id == "s0")
                .unwrap()
                .has_unread,
            Some(true)
        )
    });
}
#[gpui_kit::test]
fn target_approval_and_plan_arriving_before_list_snapshot_are_replayed(cx: &mut TestAppContext) {
    let (w, _, list, _) = open(cx, 0);
    let permission=serde_json::from_value(serde_json::json!({"type":"permission_request","sessionId":"s2","request":{"sessionId":"s2","requestId":"r","toolName":"Write","description":"Write fixture","canAlwaysAllow":true}})).unwrap();
    let plan = serde_json::from_value(
        serde_json::json!({"type":"plan_submitted","sessionId":"s2","message":{"id":"plan","role":"assistant","content":"Plan","timestamp":1}}),
    )
    .unwrap();
    cx.update(|cx| {
        list.update(cx, |s, cx| {
            s.apply_batch([&permission, &plan], cx);
            assert!(s.model.refresh_required);
            s.replace(vec![session(2, 1)], cx);
            s.apply_batch([&permission, &plan], cx);
        })
    });
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("approval-dot:s2").is_some());
        assert!(list.read(cx).model.plans.contains("s2"));
    })
    .unwrap();
}
#[gpui_kit::test]
fn workspace_menu_dispatches_to_its_owner_from_search_focus(cx: &mut TestAppContext) {
    let (w, _, _, events) = open(cx, 3);
    cx.update_window(w, |_, w, cx| {
        w.render_frame(cx);
        w.click("session-search", cx);
        w.click("workspace-switcher", cx);
        w.within("popup-menu").click(1usize, cx);
    })
    .unwrap();
    assert!(
        events
            .borrow()
            .iter()
            .any(|e| matches!(e,SidebarIntent::SwitchWorkspace(id)if id=="other"))
    );
}
