use super::*;
use gpui_kit::component::{button::Button, *};
impl SettingsView {
    pub(super) fn general(&self, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.weak_entity();
        let zh = owner.clone();
        let en = owner.clone();
        let restore = owner.clone();
        let enter = owner.clone();
        let modified = owner;
        v_flex()
            .gap_5()
            .child(div().font_semibold().child(t!("ak.settings.language")))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("settings-zh")
                            .label(t!("ak.settings.zh"))
                            .selected(self.prefs.language == "zh-CN")
                            .on_click(move |_, _, cx| {
                                emit(
                                    &zh,
                                    SettingsIntent::Local(LocalChange::Language("zh-CN".into())),
                                    cx,
                                )
                            }),
                    )
                    .child(
                        Button::new("settings-en")
                            .label(t!("ak.settings.en"))
                            .selected(self.prefs.language == "en")
                            .on_click(move |_, _, cx| {
                                emit(
                                    &en,
                                    SettingsIntent::Local(LocalChange::Language("en".into())),
                                    cx,
                                )
                            }),
                    ),
            )
            .child(
                Button::new("settings-restore")
                    .label(t!("ak.settings.restore"))
                    .selected(self.prefs.restore)
                    .on_click({
                        let next = !self.prefs.restore;
                        move |_, _, cx| {
                            emit(
                                &restore,
                                SettingsIntent::Local(LocalChange::Restore(next)),
                                cx,
                            )
                        }
                    }),
            )
            .child(div().font_semibold().child(t!("ak.settings.send_key")))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("settings-send-enter")
                            .label(t!("ak.compose.key_enter"))
                            .selected(self.prefs.send_key == SendKey::Enter)
                            .on_click(move |_, _, cx| {
                                emit(
                                    &enter,
                                    SettingsIntent::Local(LocalChange::SendKey(SendKey::Enter)),
                                    cx,
                                )
                            }),
                    )
                    .child(
                        Button::new("settings-send-mod")
                            .label(if cfg!(target_os = "macos") {
                                t!("ak.compose.key_cmd_enter_mac")
                            } else {
                                t!("ak.compose.key_cmd_enter")
                            })
                            .selected(self.prefs.send_key == SendKey::CmdEnter)
                            .on_click(move |_, _, cx| {
                                emit(
                                    &modified,
                                    SettingsIntent::Local(LocalChange::SendKey(SendKey::CmdEnter)),
                                    cx,
                                )
                            }),
                    ),
            )
            .into_any_element()
    }
    pub(super) fn appearance(&self, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.weak_entity();
        let fade = owner.clone();
        let enabled = !self.prefs.stream_fade;
        v_flex()
            .gap_5()
            .child(div().font_semibold().child(t!("ak.settings.theme")))
            .child(
                h_flex().gap_2().children(
                    [
                        (Appearance::System, t!("ak.settings.system")),
                        (Appearance::Light, t!("ak.settings.light")),
                        (Appearance::Dark, t!("ak.settings.dark")),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(i, (mode, label))| {
                        let owner = owner.clone();
                        Button::new(("settings-theme", i))
                            .label(label)
                            .selected(self.prefs.appearance == mode)
                            .on_click(move |_, _, cx| {
                                emit(
                                    &owner,
                                    SettingsIntent::Local(LocalChange::Appearance(mode)),
                                    cx,
                                )
                            })
                    }),
                ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("ak.settings.bundled_themes")),
            )
            .child(div().font_semibold().child(t!("ak.settings.font")))
            .child(h_flex().gap_2().children((12u8..=16).map(|value| {
                let owner = owner.clone();
                Button::new(("settings-font", value as usize))
                    .label(value.to_string())
                    .selected(self.prefs.font_size == value)
                    .on_click(move |_, _, cx| {
                        emit(&owner, SettingsIntent::Local(LocalChange::Font(value)), cx)
                    })
            })))
            .child(
                Button::new("settings-fade")
                    .label(t!("ak.settings.fade"))
                    .selected(self.prefs.stream_fade)
                    .on_click(move |_, _, cx| {
                        emit(&fade, SettingsIntent::Local(LocalChange::Fade(enabled)), cx)
                    }),
            )
            .into_any_element()
    }
}
