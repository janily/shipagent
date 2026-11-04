use super::{bridge::Request, confirm_view::Confirmation, *};
use gpui_kit::component::{Icon, IconName, WindowExt};
use gpui_kit::{prelude::*, *};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
pub(super) async fn run(
    request: &Request,
    spec: ConfirmSpec,
    handle: AnyWindowHandle,
    cx: &mut AsyncApp,
    cancel_on_window_close: bool,
) {
    let cancel = cancel_on_window_close.then(|| spec.cancel_index());
    // Don't cover an existing app modal; cancellation while queued performs no native action.
    loop {
        if !request.live() {
            return;
        }
        match handle.update(cx, |_, window, cx| {
            if window.root::<gpui_kit::base::Root>().flatten().is_none() {
                return Err(unavailable("Window has no native dialog host"));
            }
            Ok(window.has_active_dialog(cx))
        }) {
            Ok(Ok(false)) => break,
            Ok(Ok(true)) => {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(20))
                    .await
            }
            Ok(Err(e)) => {
                request.finish(Err(e));
                return;
            }
            Err(_) => {
                parent_closed(request, cancel);
                return;
            }
        };
    }
    let closed = Rc::new(Cell::new(false));
    let icon = match spec.kind.as_deref() {
        Some("warning") => Some(IconName::TriangleAlert),
        Some("error") => Some(IconName::CircleX),
        Some("info" | "question") => Some(IconName::Info),
        _ => None,
    };
    let title = spec.title.clone();
    let done = closed.clone();
    let alive = request.alive.clone();
    let reply = request.reply.clone();
    let opened = handle.update(cx, |_, window, cx| {
        let view = cx.new(|cx| Confirmation {
            buttons: spec.buttons.iter().map(|_| cx.focus_handle()).collect(),
            spec,
            focus: cx.focus_handle(),
            alive,
            closed: done,
            reply: RefCell::new(Some(reply)),
        });
        let default = view.read(cx).buttons[view.read(cx).spec.default_index()].clone();
        let closing = view.clone();
        let accepting = view.clone();
        let canceling = view.clone();
        let content = view.clone();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let closing = closing.clone();
            let accepting = accepting.clone();
            let canceling = canceling.clone();
            alert
                .title(title.clone())
                .when_some(icon.clone(), |a, icon| a.icon(Icon::new(icon)))
                .child(content.clone())
                .footer(div())
                .on_ok(move |_, window, cx| {
                    accepting.update(cx, |v, _| {
                        let index = v
                            .buttons
                            .iter()
                            .position(|f| f.is_focused(window))
                            .unwrap_or(v.spec.default_index());
                        v.answer(index);
                    });
                    true
                })
                .on_cancel(move |_, _, cx| {
                    canceling.update(cx, |v, _| v.answer(v.spec.cancel_index()));
                    true
                })
                .on_close(move |_, _, cx| {
                    closing.update(cx, |v, _| v.answer(v.spec.cancel_index()))
                })
        });
        window.defer(cx, move |window, cx| default.focus(window, cx));
        (view, window.focused(cx))
    });
    let Ok((view, dialog_focus)) = opened else {
        parent_closed(request, cancel);
        return;
    };
    while !closed.get() {
        let result = handle.update(cx, |_, window, cx| {
            if !request.live() {
                let own_focus = dialog_focus
                    .as_ref()
                    .is_some_and(|f| f.contains_focused(window, cx))
                    || view.read(cx).focus.contains_focused(window, cx);
                view.update(cx, |_, cx| cx.notify());
                // Never close an unrelated nested dialog; wait until our focus scope returns.
                if own_focus {
                    window.close_dialog(cx);
                    closed.set(true);
                }
            }
        });
        if result.is_err() {
            parent_closed(request, cancel);
            return;
        }
        cx.background_executor()
            .timer(std::time::Duration::from_millis(20))
            .await;
    }
    // Kit closes via deferred focus restoration and an exit transition. Keep the modal
    // lease until that transaction has settled, so the previous close cannot hit the next request.
    cx.background_executor()
        .timer(
            *gpui_kit::component::dialog::ANIMATION_DURATION + std::time::Duration::from_millis(16),
        )
        .await;
}

fn parent_closed(request: &Request, cancel: Option<usize>) {
    request.finish(match cancel {
        Some(index) => serde_json::from_value(serde_json::json!({"response":index}))
            .map_err(|_| failure("Invalid confirmation result")),
        None => Err(failure("Parent window closed")),
    });
}
