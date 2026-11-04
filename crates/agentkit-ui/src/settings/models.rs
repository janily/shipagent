use super::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    *,
};
impl SettingsView {
    pub(super) fn models(&self, cx: &mut Context<Self>) -> AnyElement {
        let ready = !self.busy && self.app.as_ref().is_some_and(|s| s.read(cx).connected);
        let connections = self
            .app
            .as_ref()
            .map(|a| a.read(cx).connections.clone())
            .unwrap_or_default();
        let rows = self.connection_rows(ready, &connections, cx);
        let owner = cx.weak_entity();
        v_flex()
            .gap_4()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("ak.settings.privacy")),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("connection-add")
                            .primary()
                            .label(t!("ak.settings.add"))
                            .disabled(!ready)
                            .on_click(
                                cx.listener(|_, _, _, cx| cx.emit(SettingsIntent::Edit(None))),
                            ),
                    )
                    .child(
                        Button::new("settings-reload")
                            .label(t!("ak.settings.reload"))
                            .disabled(self.busy)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsIntent::Reload))),
                    ),
            )
            .when(connections.is_empty(), |v| {
                v.child(t!("ak.settings.no_connections"))
            })
            .children(rows)
            .child(
                div()
                    .font_semibold()
                    .child(t!("ak.settings.default_thinking")),
            )
            .child(
                h_flex().gap_2().flex_wrap().children(
                    [
                        g::ThinkingLevel::Off,
                        g::ThinkingLevel::Low,
                        g::ThinkingLevel::Medium,
                        g::ThinkingLevel::High,
                        g::ThinkingLevel::Xhigh,
                        g::ThinkingLevel::Max,
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(i, level)| {
                        let owner = owner.clone();
                        Button::new(("settings-thinking", i))
                            .small()
                            .label(thinking_name(level))
                            .selected(self.thinking == level)
                            .disabled(!ready || !self.loaded)
                            .on_click(move |_, _, cx| {
                                emit(&owner, SettingsIntent::Thinking(level), cx)
                            })
                    }),
                ),
            )
            .child(div().font_semibold().child(t!("ak.settings.compact")))
            .child(
                h_flex().gap_2().flex_wrap().children(
                    [
                        None,
                        Some(16384),
                        Some(32768),
                        Some(65536),
                        Some(131072),
                        Some(262144),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(i, tokens)| {
                        let owner = owner.clone();
                        Button::new(("settings-compact", i))
                            .small()
                            .label(tokens.map_or_else(
                                || t!("ak.settings.disabled"),
                                |n| t!("ak.settings.tokens", count = n),
                            ))
                            .selected(self.compact == tokens)
                            .disabled(!ready || !self.loaded)
                            .on_click(move |_, _, cx| {
                                emit(&owner, SettingsIntent::Compact(tokens), cx)
                            })
                    }),
                ),
            )
            .when_some(self.compact, |v, tokens| {
                v.child(
                    div()
                        .text_xs()
                        .child(t!("ak.settings.current_threshold", count = tokens)),
                )
            })
            .into_any_element()
    }
}
fn thinking_name(level: g::ThinkingLevel) -> String {
    match level {
        g::ThinkingLevel::Off => t!("ak.compose.thinking_off"),
        g::ThinkingLevel::Low => t!("ak.compose.thinking_low"),
        g::ThinkingLevel::Medium => t!("ak.compose.thinking_medium"),
        g::ThinkingLevel::High => t!("ak.compose.thinking_high"),
        g::ThinkingLevel::Xhigh => t!("ak.compose.thinking_xhigh"),
        g::ThinkingLevel::Max => t!("ak.compose.thinking_max"),
    }
}
