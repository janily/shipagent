use super::*;
use gpui_kit::component::*;
use gpui_kit::component::{
    Sizable,
    button::{Button, ButtonVariants},
    menu::DropdownMenu,
};
use serde::Deserialize;
#[derive(Clone, PartialEq, Deserialize, Action)]
#[action(namespace=composer,no_json)]
pub(super) struct ChooseModel {
    pub connection: String,
    pub model: String,
}
#[derive(Clone, PartialEq, Deserialize, Action)]
#[action(namespace=composer,no_json)]
pub(super) struct ChooseThinking {
    pub level: g::ThinkingLevel,
}
#[derive(Clone, PartialEq, Deserialize, Action)]
#[action(namespace=composer,no_json)]
pub(super) struct ChooseMode {
    pub mode: g::PermissionMode,
}
#[derive(Clone, PartialEq, Deserialize, Action)]
#[action(namespace=composer,no_json)]
pub(super) struct ChooseSendKey {
    pub key: SendKey,
}
fn mode_name(mode: g::PermissionMode) -> String {
    match mode {
        g::PermissionMode::Safe => t!("ak.safe"),
        g::PermissionMode::Ask => t!("ak.mode_ask"),
        g::PermissionMode::AllowAll => t!("ak.mode_execute"),
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
impl Composer {
    pub(super) fn mode_menu(&self, mode: g::PermissionMode, cx: &App) -> impl IntoElement {
        let disabled = !self.selectable() || self.changing;
        Button::new("composer-mode")
            .small()
            .outline()
            .dropdown_caret(true)
            .label(mode_name(mode))
            .accessibility_label(t!("ak.compose.mode_menu"))
            .disabled(disabled)
            .when(mode == g::PermissionMode::AllowAll, |b| {
                b.text_color(cx.theme().warning)
            })
            .dropdown_menu_with_anchor(Anchor::TopLeft, move |mut menu, _, _| {
                let selected = mode;
                for mode in [
                    g::PermissionMode::Safe,
                    g::PermissionMode::Ask,
                    g::PermissionMode::AllowAll,
                ] {
                    menu = menu.menu_with_check(
                        mode_name(mode),
                        selected == mode,
                        Box::new(ChooseMode { mode }),
                    );
                }
                menu
            })
    }
    pub(super) fn model_menu(
        &self,
        model: Option<String>,
        connection: Option<String>,
    ) -> impl IntoElement {
        let choices = self.choices.clone();
        let label = choices
            .iter()
            .find(|m| Some(&m.id) == model.as_ref() && Some(&m.connection) == connection.as_ref())
            .map(|m| m.name.clone())
            .or_else(|| model.clone())
            .unwrap_or_else(|| t!("ak.no_model"));
        Button::new("composer-model")
            .small()
            .outline()
            .dropdown_caret(true)
            .label(label)
            .accessibility_label(t!("ak.compose.model_menu"))
            .disabled(!self.selectable() || self.changing || choices.is_empty())
            .dropdown_menu_with_anchor(Anchor::TopLeft, move |mut menu, _, _| {
                let mut group = None;
                for m in &choices {
                    if group.as_ref() != Some(&m.connection) {
                        menu = menu.label(m.group.clone());
                        group = Some(m.connection.clone());
                    }
                    menu = menu.menu_with_check(
                        m.name.clone(),
                        Some(&m.id) == model.as_ref() && Some(&m.connection) == connection.as_ref(),
                        Box::new(ChooseModel {
                            connection: m.connection.clone(),
                            model: m.id.clone(),
                        }),
                    );
                }
                menu
            })
    }
    pub(super) fn thinking_menu(&self, thinking: g::ThinkingLevel) -> impl IntoElement {
        Button::new("composer-thinking")
            .small()
            .outline()
            .dropdown_caret(true)
            .label(thinking_name(thinking))
            .accessibility_label(t!("ak.compose.thinking_menu"))
            .disabled(!self.selectable() || self.changing)
            .dropdown_menu_with_anchor(Anchor::TopLeft, move |mut menu, _, _| {
                for level in [
                    g::ThinkingLevel::Off,
                    g::ThinkingLevel::Low,
                    g::ThinkingLevel::Medium,
                    g::ThinkingLevel::High,
                    g::ThinkingLevel::Xhigh,
                    g::ThinkingLevel::Max,
                ] {
                    menu = menu.menu_with_check(
                        thinking_name(level),
                        thinking == level,
                        Box::new(ChooseThinking { level }),
                    );
                }
                menu
            })
    }
    pub(super) fn send_key_menu(&self) -> impl IntoElement {
        let key = self.send_key;
        Button::new("composer-send-key")
            .small()
            .ghost()
            .dropdown_caret(true)
            .label(if key == SendKey::Enter {
                t!("ak.compose.key_enter")
            } else {
                secondary_key_label()
            })
            .accessibility_label(t!("ak.compose.send_key_menu"))
            .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
                menu.menu_with_check(
                    t!("ak.compose.key_enter"),
                    key == SendKey::Enter,
                    Box::new(ChooseSendKey {
                        key: SendKey::Enter,
                    }),
                )
                .menu_with_check(
                    secondary_key_label(),
                    key == SendKey::CmdEnter,
                    Box::new(ChooseSendKey {
                        key: SendKey::CmdEnter,
                    }),
                )
            })
    }
}

fn secondary_key_label() -> String {
    if cfg!(target_os = "macos") {
        t!("ak.compose.key_cmd_enter_mac")
    } else {
        t!("ak.compose.key_cmd_enter")
    }
}
