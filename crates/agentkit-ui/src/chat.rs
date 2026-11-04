use crate::{WorkbenchIntent, WorkbenchView, t};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    *,
};
use gpui_kit::{prelude::*, *};

impl WorkbenchView {
    pub(crate) fn chat_body(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let history = if self.loading {
            div().flex_1().child(t!("ak.loading")).into_any_element()
        } else {
            self.chat.clone().into_any_element()
        };
        let connections = self
            .app_state
            .as_ref()
            .map(|s| s.read(cx).connections.as_slice())
            .unwrap_or_default();
        let choices = crate::composer::model_choices(connections);
        let settings = self
            .workspace_state
            .as_ref()
            .and_then(|s| s.read(cx).settings.as_ref());
        let session = self.state.as_ref().map(|s| &s.read(cx).model.state.session);
        let (effective_model, selected_connection) = if self.app_state.is_some() {
            crate::composer::resolved_model(connections, session, settings)
        } else {
            (
                session
                    .and_then(|s| s.model.clone())
                    .or_else(|| self.model.clone()),
                None,
            )
        };
        let thinking = settings
            .and_then(|s| s.thinking_level)
            .unwrap_or(agentkit_protocol::generated::ThinkingLevel::Medium);
        let execute = session.is_some_and(|s| {
            s.permission_mode == Some(agentkit_protocol::generated::PermissionMode::AllowAll)
        });
        self.composer.update(cx, |c, cx| {
            c.configure(
                crate::composer::Config {
                    store: self.state.clone(),
                    ready: self.ready,
                    submitting: self.submitting,
                    loading: self.loading,
                    choices,
                    model: effective_model,
                    connection: selected_connection,
                    thinking,
                },
                cx,
            )
        });
        self.permission_prompt.update(cx, |p, cx| {
            p.bind(self.state.clone(), window, cx);
            p.set_ready(self.ready && !self.loading, window, cx);
        });
        v_flex()
            .flex_1()
            .min_h_0()
            .when_some(self.state.as_ref(), |v, s| {
                v.child(crate::permissions::render_notices(s.read(cx), None, cx))
            })
            .child(history)
            .when(
                self.state
                    .as_ref()
                    .is_some_and(|s| !s.read(cx).model.permissions.is_empty()),
                |v| {
                    v.child(
                        h_flex()
                            .justify_center()
                            .px_6()
                            .py_2()
                            .child(self.permission_prompt.clone()),
                    )
                },
            )
            .when(execute, |v| {
                v.child(
                    h_flex().justify_center().px_6().child(
                        h_flex()
                            .id("execute-warning")
                            .w_full()
                            .max_w(px(820.))
                            .gap_2()
                            .p_2()
                            .border_1()
                            .border_color(cx.theme().warning)
                            .rounded_md()
                            .child(Icon::new(IconName::Info))
                            .child(div().flex_1().child(t!("ak.approval.execute_warning")))
                            .child(
                                Button::new("return-safe")
                                    .ghost()
                                    .label(t!("ak.approval.return_safe"))
                                    .disabled(
                                        !self.ready
                                            || self.loading
                                            || self
                                                .state
                                                .as_ref()
                                                .is_some_and(|s| s.read(cx).selection_pending),
                                    )
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(WorkbenchIntent::ModeChanged(
                                            agentkit_protocol::generated::PermissionMode::Safe,
                                        ))
                                    })),
                            )
                            .test_support(),
                    ),
                )
            })
            .child(
                h_flex()
                    .justify_center()
                    .px_6()
                    .py_4()
                    .child(self.composer.clone()),
            )
            .into_any_element()
    }
}
