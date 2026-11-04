use super::*;
impl Composer {
    pub(super) fn enter(
        &mut self,
        action: &gpui_kit::component::input::Enter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.input.read(cx).focus_handle(cx).is_focused(window) {
            cx.propagate();
            return;
        }
        if self.composing(window, cx) {
            cx.stop_propagation();
            return;
        }
        if !action.shift && (self.send_key == SendKey::Enter || action.secondary) {
            self.submit(window, cx);
            cx.stop_propagation();
        } else {
            cx.propagate();
        }
    }
    pub(super) fn escape(
        &mut self,
        _: &gpui_kit::component::input::Escape,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.input.read(cx).focus_handle(cx).is_focused(window)
            && !self.composing(window, cx)
            && self.session(cx).is_some_and(|s| s.is_processing)
        {
            self.stop(cx);
            cx.stop_propagation();
        } else {
            cx.propagate();
        }
    }
    pub(super) fn history(
        &mut self,
        _: &gpui_kit::component::input::MoveUp,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.input.read(cx).focus_handle(cx).is_focused(window)
            && self.input.read(cx).value().is_empty()
            && !self.composing(window, cx)
            && let Some(text) = self
                .session(cx)
                .and_then(|s| {
                    s.messages
                        .iter()
                        .rev()
                        .find(|m| m.role == g::MessageRole::User)
                })
                .map(|m| m.content.clone())
        {
            self.input
                .update(cx, |input, cx| input.set_value(text, window, cx));
            cx.stop_propagation();
            return;
        }
        cx.propagate();
    }
}
