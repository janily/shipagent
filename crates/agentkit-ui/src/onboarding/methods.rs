use super::*;
use gpui_kit::component::{button::Button, *};
impl OnboardingView {
    pub(super) fn methods_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let choices = [
            (
                Some(ConnectionKind::Preset),
                t!("ak.settings.preset"),
                t!("ak.onboard.preset_hint"),
            ),
            (
                Some(ConnectionKind::OpenAi),
                t!("ak.settings.openai"),
                t!("ak.onboard.openai_hint"),
            ),
            (
                Some(ConnectionKind::Anthropic),
                t!("ak.settings.anthropic"),
                t!("ak.onboard.anthropic_hint"),
            ),
            (
                Some(ConnectionKind::Ollama),
                t!("ak.settings.ollama"),
                t!("ak.onboard.ollama_hint"),
            ),
            (None, t!("ak.onboard.claude"), t!("ak.onboard.oauth_later")),
            (None, t!("ak.onboard.chatgpt"), t!("ak.oauth.waiting")),
        ];
        v_flex()
            .gap_5()
            .child(t!("ak.onboard.need_model"))
            .child(
                div()
                    .grid()
                    .grid_cols(3)
                    .gap_3()
                    .children(
                        choices
                            .into_iter()
                            .enumerate()
                            .map(|(i, (kind, label, hint))| {
                                v_flex()
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .rounded_lg()
                                    .p_4()
                                    .gap_3()
                                    .child(
                                        Button::new(("onboarding-method", i))
                                            .label(label)
                                            .disabled(self.busy || (kind.is_none() && i != 5))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if i == 5 {
                                                    this.busy = true;
                                                    cx.emit(OnboardingIntent::ChatGpt);
                                                    cx.notify();
                                                } else if let Some(kind) = kind {
                                                    this.busy = true;
                                                    cx.emit(OnboardingIntent::Choose(kind));
                                                    cx.notify();
                                                }
                                            })),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(hint),
                                    )
                            }),
                    ),
            )
            .child(
                Button::new("onboarding-welcome")
                    .label(t!("ak.onboard.previous"))
                    .disabled(self.busy)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.step = 0;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}
