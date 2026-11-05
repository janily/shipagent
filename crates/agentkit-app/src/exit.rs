//! Exit confirmation is global so sheets and text-input focus cannot bypass it.
use crate::actions::ConfirmedQuit;
use agentkit_ui::t;
use gpui_kit::{component::WindowExt, *};
#[derive(Default)]
pub(crate) struct ExitState {
    list: Option<WeakEntity<agentkit_store::entities::SessionListStore>>,
    stores: Vec<WeakEntity<agentkit_store::entities::SessionStore>>,
    prompt: bool,
}
impl Global for ExitState {}
pub(crate) fn allow(cx: &mut App) -> bool {
    let state = cx.global::<ExitState>();
    let busy = state
        .list
        .as_ref()
        .and_then(|s| s.upgrade())
        .is_some_and(|s| s.read(cx).model.sessions.iter().any(|s| s.is_processing))
        || state
            .stores
            .iter()
            .filter_map(|s| s.upgrade())
            .any(|s| s.read(cx).model.busy());
    if !busy {
        return true;
    }
    if cx.global::<ExitState>().prompt {
        return false;
    }
    let Some(handle) = cx.active_window().or_else(|| cx.windows().last().copied()) else {
        return false;
    };
    let _ = handle.update(cx, |_, window, cx| {
        if window.has_active_dialog(cx) {
            return;
        }
        cx.global_mut::<ExitState>().prompt = true;
        window.open_alert_dialog(cx, |alert, _, _| {
            alert
                .title(t!("ak.menu.quit"))
                .description(t!("ak.menu.quit_confirm"))
                .cancel_text(t!("ak.menu.keep_working"))
                .ok_text(t!("ak.menu.quit"))
                .on_ok(|_, window, cx| {
                    window.dispatch_action(Box::new(ConfirmedQuit), cx);
                    true
                })
                .on_close(|_, _, cx| cx.global_mut::<ExitState>().prompt = false)
        });
    });
    false
}

pub(crate) fn track(
    list: WeakEntity<agentkit_store::entities::SessionListStore>,
    stores: Vec<WeakEntity<agentkit_store::entities::SessionStore>>,
    cx: &mut App,
) {
    if cx.try_global::<ExitState>().is_some() {
        let state = cx.global_mut::<ExitState>();
        state.list = Some(list);
        state.stores = stores;
    }
}
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use ::core::prelude::v1::test;
    use gpui_kit::test::TestWindowExt;
    struct Empty;
    impl Render for Empty {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }
    #[gpui_kit::test]
    fn quit_reads_current_session_state_without_waiting_for_a_parent_render(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_global(ExitState::default());
        });
        let (w,store)=cx.update(|cx|{
        let (w,_)=gpui_kit::open_window(WindowOptions::default(),cx,|_,cx|cx.new(|_|Empty)).unwrap();
        let session=serde_json::from_value(serde_json::json!({"id":"s","workspaceId":"w","workspaceName":"W","messages":[],"lastMessageAt":0,"isProcessing":false})).unwrap();
        let store=cx.new(|_|agentkit_store::entities::SessionStore::new(session));
        let list=cx.new(|_|agentkit_store::entities::SessionListStore::default());track(list.downgrade(),vec![store.downgrade()],cx);(w,store)
    });
        cx.update(|cx| {
            store.update(cx, |s, _| s.model.state.session.is_processing = true);
            assert!(!allow(cx));
        });
        cx.update_window(w, |_, w, cx| {
            w.render_frame(cx);
            assert!(w.has_active_dialog(cx));
            w.press("escape", cx);
        })
        .unwrap();
        cx.update(|cx| {
            store.update(cx, |s, _| s.model.state.session.is_processing = false);
            assert!(allow(cx));
        });
    }
}
