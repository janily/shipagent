use super::*;
impl WorkbenchView {
    pub(super) fn sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let active = self
            .state
            .as_ref()
            .map(|s| s.read(cx).model.state.session.id.clone());
        self.sidebar_view.update(cx, |s, cx| {
            s.set_context(active, self.ready && !self.loading, cx)
        });
        v_flex()
            .size_full()
            .child(div().flex_1().min_h_0().child(self.sidebar_view.clone()))
            .child(
                Button::new("model-settings")
                    .ghost()
                    .label(t!("ak.settings"))
                    .disabled(self.busy(cx))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.show_configuration(window, cx)),
                    ),
            )
            .child(
                Button::new("toggle-theme")
                    .ghost()
                    .label(t!("ak.theme"))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(WorkbenchIntent::CycleAppearance))),
            )
            .into_any_element()
    }
}
