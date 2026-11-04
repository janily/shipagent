use super::*;
impl WorkbenchView {
    fn status_label(&self) -> String {
        match self.status_key {
            "ak.connected" => t!("ak.connected"),
            "ak.disconnected" => t!("ak.disconnected"),
            _ => t!("ak.starting"),
        }
    }

    pub(crate) fn render_shell(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.settings_open {
            return v_flex()
                .size_full()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .child(
                    TitleBar::new()
                        .h_10()
                        .child(div().flex_1().text_center().child(t!("ak.settings"))),
                )
                .when(!self.preferences_writable, |v| {
                    v.child(
                        div()
                            .text_color(cx.theme().danger)
                            .child(t!("ak.prefs_readonly")),
                    )
                })
                .child(div().flex_1().min_h_0().child(self.settings_view.clone()))
                .into_any_element();
        }
        let width = f32::from(window.bounds().size.width);
        let left = self.shell.sidebar_visible(width);
        let right = self.shell.right_visible() && !self.shell.right_is_sheet(width);
        let sidebar = self.sidebar(cx);
        let center = self.center(window, cx);
        let panel = self.right_panel(cx);
        let weak = cx.weak_entity();
        let columns = h_resizable("workbench-columns")
            .with_state(&self.panel_sizes)
            .child(
                resizable_panel()
                    .visible(left)
                    .size(px(self.shell.sidebar_width))
                    .fixed()
                    .size_range(px(240.)..px(420.))
                    .child(sidebar),
            )
            .child(
                resizable_panel()
                    .size_range(px(320.)..px(16384.))
                    .child(center),
            )
            .child(
                resizable_panel()
                    .visible(right)
                    .size(px(self.shell.right_width))
                    .fixed()
                    .size_range(px(280.)..px(560.))
                    .child(panel),
            )
            .on_resize(move |sizes, _, cx| {
                let values = sizes.read(cx).sizes().clone();
                let _ = weak.update(cx, |v, cx| {
                    if left && let Some(w) = values.first() {
                        v.shell.sidebar_width = f32::from(*w);
                    }
                    if right && let Some(w) = values.get(2) {
                        v.shell.right_width = f32::from(*w);
                    }
                    v.shell.validate();
                    v.persist_shell(cx);
                    cx.notify();
                });
            });
        let version = self
            .app_state
            .as_ref()
            .and_then(|s| s.read(cx).server_version.clone())
            .unwrap_or_else(|| "—".into());
        let tasks = self
            .app_state
            .as_ref()
            .map_or(0, |s| s.read(cx).background_tasks);
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_size(px(self.font_size as f32))
            .child(
                TitleBar::new().h_10().child(
                    h_flex()
                        .flex_1()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("toggle-sidebar")
                                .ghost()
                                .icon(IconName::PanelLeft)
                                .accessibility_label(t!("ak.toggle_sidebar"))
                                .on_click(
                                    cx.listener(|v, _, window, cx| v.toggle_sidebar(window, cx)),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_center()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{} — {}", self.app_name, self.workspace)),
                        )
                        .child(
                            Button::new("toggle-focus")
                                .ghost()
                                .label(t!("ak.focus_mode"))
                                .accessibility_label(t!("ak.focus_mode"))
                                .on_click(
                                    cx.listener(|v, _, window, cx| v.toggle_focus_mode(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("toggle-right-panel")
                                .ghost()
                                .icon(IconName::PanelRight)
                                .accessibility_label(t!("ak.context_panel"))
                                .on_click(
                                    cx.listener(|v, _, window, cx| {
                                        v.toggle_right_panel(window, cx)
                                    }),
                                ),
                        ),
                ),
            )
            .child(div().flex_1().min_h_0().child(columns))
            .child(
                StatusBar::new()
                    .bg(cx.theme().sidebar)
                    .h_6()
                    .py_0()
                    .left(
                        div()
                            .id("connection-status")
                            .role(Role::Status)
                            .aria_label(self.status_label())
                            .child(self.status_label())
                            .test_support(),
                    )
                    .child(
                        div()
                            .id("background-task-count")
                            .role(Role::Status)
                            .aria_label(t!("ak.task_count", count = tasks))
                            .child(t!("ak.task_count", count = tasks))
                            .test_support(),
                    )
                    .right(div().text_xs().child(format!(
                        "{} · {} {version}",
                        self.model.clone().unwrap_or_else(|| t!("ak.no_model")),
                        t!("ak.backend")
                    )))
                    .when(!self.preferences_writable, |bar| {
                        bar.right(
                            div()
                                .id("preferences-status")
                                .role(Role::Alert)
                                .aria_label(t!("ak.prefs_readonly"))
                                .child(t!("ak.prefs_readonly"))
                                .test_support(),
                        )
                    }),
            )
            .into_any_element()
    }
}
