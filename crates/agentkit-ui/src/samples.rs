//! T-0-10: six native Kit components with local state and no backend IO.
use crate::t;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    dialog::DialogFooter,
    input::{InputEvent, Textarea, TextareaState},
    resizable::{h_resizable, resizable_panel},
    setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    sidebar::{Sidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
    *,
};
use gpui_kit::{prelude::*, test::TestSupportExt, *};

/// Independent component playground. Its draft and settings live only in memory.
pub struct ComponentSamples {
    draft: Entity<TextareaState>,
    messages: Vec<SharedString>,
    tool_open: bool,
    settings: bool,
    _input: Subscription,
}

impl ComponentSamples {
    /// Construct the six component examples without connecting to any service.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let draft = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder(t!("ak.samples.placeholder"))
                .auto_grow(2, 5)
                .submit_on_enter(true)
        });
        let input = cx.subscribe_in(&draft, window, |this, _, event, window, cx| {
            if matches!(event, InputEvent::PressEnter { shift: false, .. }) {
                this.submit(window, cx);
            }
        });
        Self {
            draft,
            messages: vec![],
            tool_open: false,
            settings: false,
            _input: input,
        }
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.draft.read(cx).value().to_string();
        if text.trim().is_empty() {
            return;
        }
        self.messages.push(text.into());
        self.draft
            .update(cx, |draft, cx| draft.set_value("", window, cx));
        cx.notify();
    }

    fn clear_dialog(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let weak = weak.clone();
            dialog
                .title(t!("ak.samples.clear_title"))
                .child(t!("ak.samples.clear_hint"))
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("sample-clear-cancel")
                                .label(t!("common.cancel"))
                                .outline()
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("sample-clear-confirm")
                                .label(t!("common.clear"))
                                .primary()
                                .on_click(move |_, window, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.messages.clear();
                                        this.draft.update(cx, |draft, cx| {
                                            draft.set_value("", window, cx)
                                        });
                                        cx.notify();
                                    });
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }

    fn settings_view(&self) -> impl IntoElement {
        div()
            .id("sample-settings-view")
            .test_support()
            .size_full()
            .child(
                Settings::new("sample-settings-pages")
                    .sidebar_width(px(170.))
                    .pages(vec![
                        SettingPage::new(t!("ak.samples.appearance"))
                            .default_open(true)
                            .groups(vec![
                                SettingGroup::new()
                                    .title(t!("ak.samples.theme"))
                                    .items(vec![
                                        SettingItem::new(
                                            t!("ak.samples.dark"),
                                            SettingField::switch(
                                                |cx: &App| cx.theme().mode.is_dark(),
                                                |dark, cx: &mut App| {
                                                    Theme::change(
                                                        if dark {
                                                            ThemeMode::Dark
                                                        } else {
                                                            ThemeMode::Light
                                                        },
                                                        None,
                                                        cx,
                                                    );
                                                },
                                            ),
                                        )
                                        .description(t!("ak.samples.apply_hint")),
                                    ]),
                            ]),
                    ]),
            )
    }

    fn conversation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .min_w_0()
            .gap_4()
            .p_5()
            .child(
                div()
                    .text_lg()
                    .font_semibold()
                    .child(t!("ak.samples.heading")),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("ak.samples.subtitle")),
            )
            .child(
                Collapsible::new()
                    .open(self.tool_open)
                    .child(
                        Button::new("sample-tool-toggle")
                            .outline()
                            .icon(if self.tool_open {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            })
                            .label(t!("ak.samples.read"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.tool_open = !this.tool_open;
                                cx.notify();
                            })),
                    )
                    .content(
                        div()
                            .id("sample-tool-result")
                            .test_support()
                            .p_3()
                            .bg(cx.theme().muted)
                            .rounded_lg()
                            .child(t!("ak.samples.result")),
                    ),
            )
            .child(
                v_flex()
                    .id("sample-history")
                    .flex_1()
                    .min_h_0()
                    .gap_3()
                    .overflow_y_scroll()
                    .when(self.messages.is_empty(), |this| {
                        this.child(
                            div()
                                .id("sample-empty")
                                .test_support()
                                .p_4()
                                .text_color(cx.theme().muted_foreground)
                                .child(t!("ak.samples.empty")),
                        )
                    })
                    .children(self.messages.iter().enumerate().map(|(index, message)| {
                        div()
                            .id(("sample-message", index))
                            .role(Role::Group)
                            .aria_label(message.clone())
                            .test_support()
                            .p_3()
                            .bg(cx.theme().muted)
                            .rounded_lg()
                            .child(message.clone())
                    })),
            )
            .child(
                v_flex()
                    .gap_2()
                    .p_3()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_lg()
                    .child(
                        div()
                            .id("sample-composer")
                            .test_support()
                            .child(Textarea::new(&self.draft)),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(t!("ak.samples.memory")),
                            )
                            .child(
                                Button::new("sample-send")
                                    .primary()
                                    .label(t!("ak.samples.send"))
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.submit(window, cx)),
                                    ),
                            ),
                    ),
            )
    }
}

impl Render for ComponentSamples {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(TitleBar::new().child(t!("ak.samples.title")))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        Sidebar::new("sample-sidebar")
                            .w(px(220.))
                            .header(SidebarHeader::new().child(t!("ak.samples.workspace")))
                            .child(
                                SidebarGroup::new(t!("ak.samples.group")).child(
                                    SidebarMenu::new().child(
                                        SidebarMenuItem::new(t!("ak.samples.conversation"))
                                            .icon(IconName::Bot)
                                            .active(!self.settings)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.settings = false;
                                                cx.notify();
                                            })),
                                    ),
                                ),
                            )
                            .footer(
                                SidebarFooter::new().child(
                                    Button::new("sample-settings")
                                        .ghost()
                                        .icon(IconName::Settings)
                                        .label(t!("ak.samples.settings"))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.settings = true;
                                            cx.notify();
                                        })),
                                ),
                            ),
                    )
                    .child(
                        v_flex()
                            .h_full()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .child(
                                h_flex()
                                    .h(px(52.))
                                    .flex_shrink_0()
                                    .px_4()
                                    .items_center()
                                    .justify_between()
                                    .border_b_1()
                                    .border_color(cx.theme().border)
                                    .child(
                                        Button::new("sample-back")
                                            .ghost()
                                            .label(t!("ak.samples.back"))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.settings = false;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("sample-clear")
                                            .outline()
                                            .label(t!("ak.samples.clear_samples"))
                                            .on_click(cx.listener(Self::clear_dialog)),
                                    ),
                            )
                            .child(div().flex_1().min_h_0().child(if self.settings {
                                self.settings_view().into_any_element()
                            } else {
                                h_resizable("sample-panels")
                                    .child(
                                        resizable_panel()
                                            .size_range(px(400.)..Pixels::MAX)
                                            .child(self.conversation(cx)),
                                    )
                                    .child(
                                        resizable_panel()
                                            .size(px(240.))
                                            .size_range(px(200.)..px(360.))
                                            .child(
                                                v_flex()
                                                    .id("sample-inspector")
                                                    .test_support()
                                                    .size_full()
                                                    .p_4()
                                                    .gap_4()
                                                    .child(
                                                        div()
                                                            .font_semibold()
                                                            .child(t!("ak.samples.inspector")),
                                                    )
                                                    .child("README.md")
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child(t!("ak.samples.resize_hint")),
                                                    ),
                                            ),
                                    )
                                    .into_any_element()
                            })),
                    ),
            )
            .child(
                h_flex()
                    .h(px(28.))
                    .px_3()
                    .items_center()
                    .bg(cx.theme().muted)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("ak.samples.status")),
            )
    }
}
