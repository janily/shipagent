use super::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    *,
};
impl SettingsView {
    pub(super) fn connection_rows(
        &self,
        ready: bool,
        connections: &[g::LlmConnectionWithStatus],
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        for c in connections {
            let id = c.slug.clone();
            let owner = cx.weak_entity();
            let test = owner.clone();
            let default = owner.clone();
            let refresh = owner.clone();
            let delete = owner.clone();
            let edit_id = id.clone();
            let test_id = id.clone();
            let default_id = id.clone();
            let refresh_id = id.clone();
            let delete_id = id.clone();
            rows.push(
                v_flex()
                    .p_3()
                    .gap_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().flex_1().font_semibold().child(c.name.clone()))
                            .when(c.is_default == Some(true), |v| {
                                v.child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().primary)
                                        .child(t!("ak.settings.default")),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(if c.is_authenticated {
                                cx.theme().success
                            } else {
                                cx.theme().danger
                            })
                            .child(if c.is_authenticated {
                                t!("ak.settings.authenticated")
                            } else {
                                c.auth_error
                                    .clone()
                                    .unwrap_or_else(|| t!("ak.settings.not_authenticated"))
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{} · {}", c.provider_type, c.auth_type)),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .child(
                                Button::new(SharedString::from(format!("connection-edit:{id}")))
                                    .small()
                                    .label(t!("ak.settings.edit"))
                                    .disabled(!ready)
                                    .on_click(move |_, _, cx| {
                                        emit(
                                            &owner,
                                            SettingsIntent::Edit(Some(edit_id.clone())),
                                            cx,
                                        )
                                    }),
                            )
                            .child(
                                Button::new(SharedString::from(format!("connection-test:{id}")))
                                    .small()
                                    .label(
                                        if c.auth_type == g::LlmConnectionWithStatusAuthType::Oauth
                                        {
                                            t!("ak.settings.check_auth")
                                        } else {
                                            t!("ak.settings.test")
                                        },
                                    )
                                    .disabled(!ready)
                                    .on_click(move |_, _, cx| {
                                        emit(&test, SettingsIntent::Test(test_id.clone()), cx)
                                    }),
                            )
                            .child(
                                Button::new(SharedString::from(format!("connection-default:{id}")))
                                    .small()
                                    .label(t!("ak.settings.set_default"))
                                    .disabled(!ready || c.is_default == Some(true))
                                    .on_click(move |_, _, cx| {
                                        emit(
                                            &default,
                                            SettingsIntent::Default(default_id.clone()),
                                            cx,
                                        )
                                    }),
                            )
                            .child(
                                Button::new(SharedString::from(format!("connection-refresh:{id}")))
                                    .small()
                                    .label(t!("ak.settings.refresh"))
                                    .disabled(!ready)
                                    .on_click(move |_, _, cx| {
                                        emit(
                                            &refresh,
                                            SettingsIntent::Refresh(refresh_id.clone()),
                                            cx,
                                        )
                                    }),
                            )
                            .child(
                                Button::new(SharedString::from(format!("connection-delete:{id}")))
                                    .small()
                                    .ghost()
                                    .text_color(cx.theme().danger)
                                    .label(t!("ak.settings.delete"))
                                    .disabled(!ready)
                                    .on_click(move |_, _, cx| {
                                        emit(&delete, SettingsIntent::Delete(delete_id.clone()), cx)
                                    }),
                            ),
                    )
                    .into_any_element(),
            );
        }
        rows
    }
}
