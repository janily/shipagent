use super::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    *,
};
use secrecy::SecretString;
/// Secret-bearing gestures; deliberately have no Debug implementation.
#[derive(Clone)]
pub enum ClaudeIntent {
    /// One trimmed code, removed from the editor immediately.
    Submit(SecretString),
    /// Cancel even while a code is being exchanged.
    Cancel,
}
/// Shared native masked code form hosted in the Kit Sheet for settings and onboarding.
pub struct ClaudeLoginForm {
    code: Entity<InputState>,
    _input_subscription: Subscription,
    ready: bool,
    submitted: bool,
}
impl EventEmitter<ClaudeIntent> for ClaudeLoginForm {}
impl ClaudeLoginForm {
    /// Create an empty, masked input while browser launch is pending.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let code = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder(t!("ak.claude.code"))
        });
        let subscription = cx.subscribe_in(&code, window, |this, _, event, window, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.submit(window, cx);
            }
        });
        Self {
            code,
            _input_subscription: subscription,
            ready: false,
            submitted: false,
        }
    }

    /// Enable submission only after successful browser launch.
    pub fn ready(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ready = true;
        self.code.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }
    /// Drop the text on close, cancellation, or connection replacement.
    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.code.update(cx, |s, cx| s.set_value("", window, cx));
        self.ready = false;
        cx.notify();
    }
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.ready || self.submitted {
            return;
        }
        let code = self.code.read(cx).value().trim().to_string();
        if code.is_empty() {
            return;
        }
        self.clear(window, cx);
        self.submitted = true;
        cx.emit(ClaudeIntent::Submit(code.into()));
        cx.notify();
    }
}
impl Render for ClaudeLoginForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("claude-login-form")
            .test_support()
            .gap_4()
            .p_4()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("ak.claude.instructions")),
            )
            .child(
                Input::new(&self.code)
                    .id("claude-code")
                    .disabled(!self.ready || self.submitted),
            )
            .when(self.submitted, |v| v.child(t!("ak.claude.exchanging")))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("claude-submit")
                            .primary()
                            .label(t!("ak.claude.submit"))
                            .disabled(!self.ready || self.submitted)
                            .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
                    )
                    .child(
                        Button::new("claude-cancel")
                            .label(t!("ak.oauth.cancel"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.clear(window, cx);
                                cx.emit(ClaudeIntent::Cancel);
                            })),
                    ),
            )
    }
}
