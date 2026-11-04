use super::*;
use gpui_kit::component::{
    Disableable, WindowExt,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};
use gpui_kit::{prelude::*, *};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
pub(super) struct Confirmation {
    pub(super) spec: ConfirmSpec,
    pub(super) focus: FocusHandle,
    pub(super) buttons: Vec<FocusHandle>,
    pub(super) alive: Arc<AtomicBool>,
    pub(super) closed: Rc<Cell<bool>>,
    pub(super) reply: RefCell<Option<async_channel::Sender<bridge::Reply>>>,
}
impl Confirmation {
    pub(super) fn answer(&self, index: usize) {
        self.closed.set(true);
        if let Some(reply) = self.reply.borrow_mut().take()
            && self.alive.load(Ordering::Acquire)
        {
            let value = serde_json::from_value(serde_json::json!({"response":index}))
                .map_err(|_| failure("Invalid confirmation result"));
            let _ = reply.try_send(value);
        }
    }
}
impl Render for Confirmation {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let alive = self.alive.load(Ordering::Acquire);
        v_flex()
            .id("client-confirm")
            .test_support()
            .track_focus(&self.focus)
            .gap_4()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    this.answer(this.spec.cancel_index());
                    window.close_dialog(cx);
                    cx.stop_propagation();
                }
            }))
            .child(self.spec.message.clone())
            .when_some(self.spec.detail.clone(), |v, d| v.child(d))
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .children(self.spec.buttons.iter().enumerate().map(|(index, label)| {
                        Button::new(("client-confirm-button", index))
                            .label(label.clone())
                            .track_focus(&self.buttons[index])
                            .when(index == self.spec.default_index(), |b| b.primary())
                            .disabled(!alive)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.answer(index);
                                window.close_dialog(cx);
                            }))
                    })),
            )
    }
}
