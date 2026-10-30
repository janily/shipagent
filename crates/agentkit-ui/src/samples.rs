//! T-0-10: six native Kit components with local state and no backend IO.
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
                .placeholder("输入消息 · Enter 发送，Shift+Enter 换行")
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
                .title("清空组件样板？")
                .child("清除本窗口中的样板消息与草稿。")
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("sample-clear-cancel")
                                .label("取消")
                                .outline()
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("sample-clear-confirm")
                                .label("清空")
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
                    .pages(vec![SettingPage::new("外观").default_open(true).groups(
                        vec![SettingGroup::new().title("主题").items(vec![
                                SettingItem::new(
                                    "深色模式",
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
                                .description("即时应用到本次样板窗口"),
                            ])],
                    )]),
            )
    }

    fn conversation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .min_w_0()
            .gap_4()
            .p_5()
            .child(div().text_lg().font_semibold().child("原生组件工作台"))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("GPUI Kit 0.8.1 · 六类组件交互样板"),
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
                            .label("读取 README.md · 0.1s")
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
                            .child("# ShipAgent\n原生 Agent 工作台 · 工具结果样板"),
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
                                .child("输入一条消息，验证原生 Textarea"),
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
                                    .child("仅在内存中保留"),
                            )
                            .child(Button::new("sample-send").primary().label("发送").on_click(
                                cx.listener(|this, _, window, cx| this.submit(window, cx)),
                            )),
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
            .child(TitleBar::new().child("ShipAgent · 组件样板"))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        Sidebar::new("sample-sidebar")
                            .w(px(220.))
                            .header(SidebarHeader::new().child("默认工作区"))
                            .child(
                                SidebarGroup::new("组件样板").child(
                                    SidebarMenu::new().child(
                                        SidebarMenuItem::new("原生会话")
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
                                        .label("设置")
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
                                            .label("返回工作台")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.settings = false;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("sample-clear")
                                            .outline()
                                            .label("清空样板")
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
                                                        div().font_semibold().child("文件与计划"),
                                                    )
                                                    .child("README.md")
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child("拖动左侧分隔线调整宽度"),
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
                    .child("组件样板 · 本地交互"),
            )
    }
}
