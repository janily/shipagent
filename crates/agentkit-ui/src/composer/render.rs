use super::menus::*;
use super::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::Textarea,
    *,
};
impl Render for Composer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.mention_context_changed {
            self.mention_context_changed = false;
            self.refresh_mentions(window, cx);
        }
        if self.mention_catalog_changed {
            self.mention_catalog_changed = false;
            if self
                .mention_query
                .as_ref()
                .is_some_and(|q| q.kind == agentkit_store::core::mentions::Kind::Skill)
            {
                self.refresh_skills(window, cx);
            }
        }
        let session = self.session(cx);
        let mode = session
            .and_then(|s| s.permission_mode)
            .unwrap_or(g::PermissionMode::Safe);
        let thinking = session
            .and_then(|s| s.thinking_level)
            .unwrap_or(self.default_thinking);
        let model = session
            .and_then(|s| s.model.clone())
            .or_else(|| self.fallback_model.clone());
        let connection = session
            .and_then(|s| s.llm_connection.clone())
            .or_else(|| self.default_connection.clone());
        let busy = self.store.as_ref().is_some_and(|s| s.read(cx).model.busy());
        let processing = session.is_some_and(|s| s.is_processing);
        let permission = self
            .store
            .as_ref()
            .is_some_and(|s| !s.read(cx).model.permissions.is_empty());
        let enabled = self.enabled();
        let placeholder = if !self.ready {
            t!("ak.compose.disconnected")
        } else if self.loading {
            t!("ak.loading")
        } else if self.fallback_model.is_none() && self.choices.is_empty() {
            t!("ak.compose.no_connection")
        } else if self.fallback_model.is_none() {
            t!("ak.compose.choose_model")
        } else if permission {
            t!("ak.compose.waiting")
        } else if busy {
            t!("ak.compose.queue_placeholder")
        } else {
            t!("ak.compose.placeholder")
        };
        self.input.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx)
        });
        let changing = self.changing;
        v_flex()
            .w_full()
            .max_w(px(820.))
            .border_1()
            .border_color(cx.theme().border)
            .rounded_lg()
            .p_3()
            .gap_3()
            .key_context("Composer")
            .capture_action(cx.listener(Self::paste))
            .capture_action(cx.listener(Self::enter))
            .capture_action(cx.listener(Self::escape))
            .capture_action(cx.listener(Self::history))
            .capture_action(cx.listener(Self::mention_down))
            .capture_action(cx.listener(Self::mention_tab))
            .capture_action(cx.listener(Self::mention_left))
            .capture_action(cx.listener(Self::mention_right))
            .capture_action(cx.listener(Self::cycle_mode))
            .on_action(cx.listener(|this, a: &ChooseModel, _, cx| {
                if this.selectable()
                    && !this.changing
                    && this
                        .choices
                        .iter()
                        .any(|m| m.connection == a.connection && m.id == a.model)
                {
                    this.changing = true;
                    cx.emit(WorkbenchIntent::ModelChanged {
                        connection: a.connection.clone(),
                        model: a.model.clone(),
                    });
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, a: &ChooseThinking, _, cx| {
                if this.selectable() && !this.changing {
                    this.changing = true;
                    cx.emit(WorkbenchIntent::ThinkingChanged(a.level));
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, a: &ChooseMode, _, cx| {
                if this.ready && this.store.is_some() && !this.loading && !this.changing {
                    this.changing = true;
                    cx.emit(WorkbenchIntent::ModeChanged(a.mode));
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, a: &ChooseSendKey, _, cx| {
                this.set_send_key(a.key, cx);
                cx.emit(WorkbenchIntent::SendKeyChanged(a.key));
            }))
            .child(self.mention_popover(cx))
            .child(self.attachment_row(cx))
            .child(
                div()
                    .id("composer")
                    .w_full()
                    .child(
                        Textarea::new(&self.input)
                            .aria_label(t!("ak.composer"))
                            .appearance(false)
                            .bordered(false)
                            .disabled(!enabled),
                    )
                    .test_support(),
            )
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .items_center()
                    .child(
                        Button::new("attach-files")
                            .small()
                            .ghost()
                            .label(t!("ak.attach.pick"))
                            .disabled(!enabled)
                            .on_click(cx.listener(|this, _, window, cx| this.pick(window, cx))),
                    )
                    .child(self.mode_menu(mode, cx))
                    .child(self.model_menu(model, connection))
                    .child(self.thinking_menu(thinking))
                    .child(div().flex_1())
                    .child(self.send_key_menu())
                    .when(processing, |v| {
                        v.child(
                            Button::new("stop")
                                .small()
                                .outline()
                                .label(t!("ak.stop"))
                                .disabled(!self.ready)
                                .on_click(cx.listener(|this, _, _, cx| this.stop(cx))),
                        )
                    })
                    .child(
                        Button::new("send")
                            .small()
                            .primary()
                            .label(if busy {
                                t!("ak.compose.queue")
                            } else {
                                t!("ak.send")
                            })
                            .disabled(
                                !enabled
                                    || self.submitting
                                    || changing
                                    || self.attachments_busy(cx)
                                    || (self.input.read(cx).value().trim().is_empty()
                                        && !self.has_attachments(cx)),
                            )
                            .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
                    ),
            )
    }
}
