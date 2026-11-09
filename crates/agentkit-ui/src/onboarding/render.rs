use super::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    stepper::{Stepper, StepperItem},
    *,
};
impl OnboardingView {
    fn content(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.step {
            0 => v_flex()
                .gap_5()
                .child(t!("ak.onboard.intro"))
                .child(
                    Button::new("onboarding-start")
                        .primary()
                        .label(t!("ak.onboard.begin"))
                        .on_click(cx.listener(|this, _, _, cx| this.methods(cx))),
                )
                .into_any_element(),
            1 => self.methods_content(cx),
            2 => v_flex()
                .flex_1()
                .min_h_0()
                .gap_4()
                .child(
                    Button::new("onboarding-back")
                        .label(t!("ak.onboard.previous"))
                        .disabled(self.busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.busy = true;
                            cx.emit(OnboardingIntent::Back);
                            cx.notify();
                        })),
                )
                .when_some(self.editor.clone(), |v, e| {
                    v.child(div().flex_1().min_h_0().child(e))
                })
                .into_any_element(),
            _ => v_flex()
                .gap_5()
                .child(t!("ak.onboard.ready"))
                .child(
                    Button::new("onboarding-finish")
                        .primary()
                        .label(t!("ak.onboard.first_chat"))
                        .disabled(self.busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.busy = true;
                            cx.emit(OnboardingIntent::Finish);
                            cx.notify();
                        })),
                )
                .into_any_element(),
        }
    }
}
impl Render for OnboardingView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let labels = [
            t!("ak.onboard.welcome"),
            t!("ak.onboard.choose"),
            t!("ak.onboard.configure"),
            t!("ak.onboard.done"),
        ];
        let header = h_flex()
            .gap_4()
            .child(
                Stepper::new("onboarding-steps")
                    .flex_1()
                    .selected_index(self.step)
                    .disabled(true)
                    .items(
                        labels
                            .iter()
                            .map(|label| StepperItem::new().child(label.clone())),
                    ),
            )
            .when(self.step == 1, |v| {
                v.child(
                    Button::new("onboarding-defer")
                        .ghost()
                        .label(t!("ak.onboard.later"))
                        .disabled(self.busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.busy = true;
                            cx.emit(OnboardingIntent::Defer);
                            cx.notify();
                        })),
                )
            });
        let content = self.content(cx);
        v_flex()
            .id("onboarding")
            .test_support()
            .track_focus(&self.focus)
            .size_full()
            .items_center()
            .p_6()
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(820.))
                    .h_full()
                    .gap_5()
                    .child(header)
                    .when(self.oauth_pending, |v| {
                        v.child(
                            Button::new("onboarding-oauth-cancel")
                                .label(t!("ak.oauth.cancel"))
                                .on_click(cx.listener(|_, _, _, cx| {
                                    cx.emit(OnboardingIntent::CancelChatGpt)
                                })),
                        )
                    })
                    .when_some(self.error.clone(), |v, e| {
                        v.child(
                            div()
                                .id("onboarding-error")
                                .role(Role::Alert)
                                .text_color(cx.theme().danger)
                                .child(e),
                        )
                    })
                    .child(
                        div()
                            .text_2xl()
                            .font_semibold()
                            .child(labels[self.step].clone()),
                    )
                    .child(content)
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(t!("ak.onboard.local")),
                    ),
            )
    }
}
