use super::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    setting::{SettingGroup, SettingItem, SettingPage, Settings},
    *,
};
impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let general = cx.weak_entity();
        let appearance = general.clone();
        let models = general.clone();
        v_flex()
            .id("settings-view")
            .test_support()
            .track_focus(&self.focus)
            .size_full()
            .gap_2()
            .child(
                h_flex().h_10().px_4().child(
                    Button::new("settings-back")
                        .ghost()
                        .label(t!("ak.settings.back"))
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsIntent::Back))),
                ),
            )
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
                            page_ix: if self.locale_generation == 0 { 2 } else { 0 },
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
                        ]),
                ),
            )
    }
}
