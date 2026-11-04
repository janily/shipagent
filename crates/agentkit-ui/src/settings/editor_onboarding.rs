use super::*;
use gpui_kit::component::input::InputEvent;
impl ConnectionEditor {
    /// Reuse the reviewed form in the full-window wizard, requiring a successful test.
    pub fn prepare_onboarding(
        &mut self,
        kind: ConnectionKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.onboarding = true;
        self.kind = kind;
        self.name.update(cx, |s, cx| {
            s.set_value(
                match kind {
                    ConnectionKind::Preset => t!("ak.settings.preset"),
                    ConnectionKind::OpenAi => t!("ak.settings.openai"),
                    ConnectionKind::Anthropic => t!("ak.settings.anthropic"),
                    ConnectionKind::Ollama => t!("ak.settings.ollama"),
                },
                window,
                cx,
            )
        });
        self.url.update(cx, |s, cx| {
            s.set_value(
                if kind == ConnectionKind::Ollama {
                    "http://127.0.0.1:11434/v1"
                } else {
                    ""
                },
                window,
                cx,
            )
        });
        for input in [
            &self.name,
            &self.url,
            &self.model,
            &self.model_list,
            &self.key,
        ] {
            self.input_subscriptions
                .push(cx.subscribe(input, |this, _, event, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.tested = false;
                        cx.notify();
                    }
                }));
        }
    }
    /// Acknowledged test result; subsequent changes invalidate it.
    pub fn test_passed(&mut self, cx: &mut Context<Self>) {
        self.tested = true;
        self.busy = false;
        self.error = None;
        self.notice = Some(t!("ak.settings.test_ok"));
        cx.notify();
    }
}
