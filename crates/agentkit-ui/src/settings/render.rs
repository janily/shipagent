use super::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    setting::{SettingGroup, SettingItem, SettingPage, Settings},
    *,
};
impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(form) = &self.proxy_form {
            let blocked = self.busy || !self.app.as_ref().is_some_and(|a| a.read(cx).connected);
            form.update(cx, |f, cx| f.blocked(blocked, cx));
        }
        let general = cx.weak_entity();
        let appearance = general.clone();
        let models = general.clone();
        let shortcuts = general.clone();
        let permissions = general.clone();
        let network = general.clone();
        let notifications = general.clone();
        let advanced = general.clone();
        v_flex()
            .id("settings-view")
            .test_support()
            .track_focus(&self.focus)
            .size_full()
            .gap_2()
            .child(
                h_flex()
                    .h_10()
                    .px_4()
                    .child(
                        Button::new("settings-back")
                            .ghost()
                            .label(t!("ak.settings.back"))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsIntent::Back))),
                    )
                    .child(
                        Button::new("settings-retry")
                            .label(t!("ak.settings.reload"))
                            .disabled(self.busy)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsIntent::Reload))),
                    ),
            )
            .when(self.oauth_pending, |v| {
                v.child(
                    Button::new("settings-oauth-cancel")
                        .label(t!("ak.oauth.cancel"))
                        .on_click(
                            cx.listener(|_, _, _, cx| cx.emit(SettingsIntent::CancelChatGpt)),
                        ),
                )
            })
            .when_some(self.error.clone(), |v, error| {
                v.child(
                    div()
                        .id("settings-error")
                        .role(Role::Alert)
                        .aria_label(error.clone())
                        .px_4()
                        .text_color(cx.theme().danger)
                        .child(error)
                        .test_support(),
                )
            })
            .when_some(self.notice.clone(), |v, notice| {
                v.child(
                    div()
                        .id("settings-notice")
                        .role(Role::Status)
                        .aria_label(notice.clone())
                        .px_4()
                        .text_color(cx.theme().success)
                        .child(notice)
                        .test_support(),
                )
            })
            .child(
                div().flex_1().min_h_0().child(
                    // Kit captures its search placeholder when its keyed state is created.
                    Settings::new(("native-settings", self.locale_generation))
                        .sidebar_width(px(220.))
                        .default_selected_index(gpui_kit::component::setting::SelectIndex {
                            page_ix: self
                                .selected_page
                                .unwrap_or(if self.locale_generation == 0 { 2 } else { 0 }),
                            group_ix: None,
                        })
                        .pages(vec![
                            SettingPage::new(t!("ak.settings.general")).groups(vec![
                                SettingGroup::new().items(vec![SettingItem::render(
                                    move |_, _, cx| {
                                        general
                                            .update(cx, |s, cx| s.general(cx))
                                            .unwrap_or_else(|_| div().into_any_element())
                                    },
                                )]),
                            ]),
                            SettingPage::new(t!("ak.settings.appearance")).groups(vec![
                                SettingGroup::new().items(vec![SettingItem::render(
                                    move |_, _, cx| {
                                        appearance
                                            .update(cx, |s, cx| s.appearance(cx))
                                            .unwrap_or_else(|_| div().into_any_element())
                                    },
                                )]),
                            ]),
                            SettingPage::new(t!("ak.settings.models"))
                                .default_open(true)
                                .groups(vec![SettingGroup::new().items(vec![
                                    SettingItem::render(move |_, _, cx| {
                                        models
                                            .update(cx, |s, cx| s.models(cx))
                                            .unwrap_or_else(|_| div().into_any_element())
                                    }),
                                ])]),
                            SettingPage::new(t!("ak.settings.permissions")).groups(vec![
                                SettingGroup::new().items(vec![SettingItem::render(
                                    move |_, _, cx| {
                                        permissions
                                            .update(cx, |s, cx| s.permissions(cx))
                                            .unwrap_or_else(|_| div().into_any_element())
                                    },
                                )]),
                            ]),
                            SettingPage::new(t!("ak.settings.network")).groups(vec![
                                SettingGroup::new().items(vec![SettingItem::render(
                                    move |_, _, cx| {
                                        network
                                            .update(cx, |s, _cx| s.network())
                                            .unwrap_or_else(|_| div().into_any_element())
                                    },
                                )]),
                            ]),
                            SettingPage::new(t!("ak.settings.notifications")).groups(vec![
                                SettingGroup::new().items(vec![SettingItem::render(
                                    move |_, _, cx| {
                                        notifications
                                            .update(cx, |s, cx| s.notifications(cx))
                                            .unwrap_or_else(|_| div().into_any_element())
                                    },
                                )]),
                            ]),
                            SettingPage::new(t!("ak.menu.shortcuts")).groups(vec![
                                SettingGroup::new().items(vec![SettingItem::render(
                                    move |_, _, cx| {
                                        shortcuts
                                            .update(cx, |s, cx| s.render_shortcuts(cx))
                                            .unwrap_or_else(|_| div().into_any_element())
                                    },
                                )]),
                            ]),
                            SettingPage::new(t!("ak.settings.advanced")).groups(vec![
                                SettingGroup::new().items(vec![SettingItem::render(
                                    move |_, _, cx| {
                                        advanced
                                            .update(cx, |s, cx| s.advanced(cx))
                                            .unwrap_or_else(|_| div().into_any_element())
                                    },
                                )]),
                            ]),
                        ]),
                ),
            )
    }
}

impl SettingsView {
    fn render_shortcuts(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .id("shortcut-list")
            .test_support()
            .gap_3()
            .when(self.keymap_invalid, |v| {
                v.child(
                    div()
                        .id("invalid-keymap")
                        .test_support()
                        .role(Role::Alert)
                        .text_color(cx.theme().danger)
                        .child(t!("ak.menu.invalid_keymap")),
                )
            })
            .child(div().text_sm().child(t!("ak.menu.keymap_hint")))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("open-keymap")
                            .label(t!("ak.menu.open_keymap"))
                            .on_click(
                                cx.listener(|_, _, _, cx| cx.emit(SettingsIntent::OpenKeymap)),
                            ),
                    )
                    .child(
                        Button::new("reload-keymap")
                            .label(t!("ak.menu.reload_keymap"))
                            .on_click(
                                cx.listener(|_, _, _, cx| cx.emit(SettingsIntent::ReloadKeymap)),
                            ),
                    ),
            )
            .children(self.shortcuts.iter().map(|(label, key, scope)| {
                h_flex()
                    .gap_4()
                    .child(div().flex_1().child(label.clone()))
                    .child(div().font_family("monospace").child(key.clone()))
                    .child(
                        div()
                            .w_40()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(scope.clone()),
                    )
            }))
            .into_any_element()
    }
}
