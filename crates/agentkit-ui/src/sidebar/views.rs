use super::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    *,
};
impl SessionSidebar {
    pub(super) fn view_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let counts = self
            .list
            .as_ref()
            .map(|list| {
                let s = list.read(cx);
                agentkit_store::core::views::counts(
                    &s.model.sessions,
                    &s.model.plans,
                    (!self.workspace_id.is_empty()).then_some(self.workspace_id.as_str()),
                )
            })
            .unwrap_or([0; 6]);
        h_flex()
            .id("session-views")
            .test_support()
            .flex_wrap()
            .gap_1()
            .children(View::ALL.into_iter().enumerate().map(|(i, view)| {
                let label = match view {
                    View::All => t!("ak.nav.view_all"),
                    View::Unread => t!("ak.nav.view_unread"),
                    View::Flagged => t!("ak.nav.view_flagged"),
                    View::Running => t!("ak.nav.view_running"),
                    View::Plans => t!("ak.nav.view_plans"),
                    View::Archived => t!("ak.nav.view_archived"),
                };
                let label = if view == View::Plans
                    && self.list.as_ref().is_some_and(|s| s.read(cx).plan_loading)
                {
                    format!("{label}…")
                } else if counts[i] > 0 {
                    format!("{label} {}", counts[i])
                } else {
                    label
                };
                Button::new(SharedString::from(format!("view:{i}")))
                    .ghost()
                    .small()
                    .label(label)
                    .disabled(!self.ready)
                    .when(self.view == view, |b| b.bg(cx.theme().sidebar_accent))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.view = view;
                        this.rebuild(cx);
                        if view == View::Plans {
                            cx.emit(SidebarIntent::RetryPlans);
                        }
                    }))
            }))
    }
    pub(super) fn search_status(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let row = h_flex()
            .gap_1()
            .text_xs()
            .text_color(cx.theme().muted_foreground);
        if self.search_error {
            row.child(t!("ak.nav.search_failed")).child(
                Button::new("search-retry")
                    .ghost()
                    .small()
                    .label(t!("ak.draft.retry"))
                    .disabled(!self.ready)
                    .on_click(cx.listener(|this, _, _, cx| this.query_changed(cx))),
            )
        } else if self.list.as_ref().is_some_and(|s| s.read(cx).plan_error) {
            row.child(t!("ak.nav.plans_failed")).child(
                Button::new("plans-retry")
                    .ghost()
                    .small()
                    .label(t!("ak.draft.retry"))
                    .disabled(!self.ready)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(SidebarIntent::RetryPlans))),
            )
        } else if self.view == View::Plans
            && self.list.as_ref().is_some_and(|s| s.read(cx).plan_loading)
        {
            row.child(t!("ak.nav.plans_loading"))
        } else if self.searching {
            row.child(t!("ak.nav.searching"))
        } else {
            row
        }
    }
    pub(super) fn content_row(
        &self,
        session: &g::Session,
        snippet: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = session.id.clone();
        let query = self.search.read(cx).value().trim().to_string();
        Button::new(SharedString::from(format!("content-session:{id}")))
            .ghost()
            .h(px(52.))
            .w_full()
            .min_w_0()
            .justify_start()
            .accessibility_label(format!(
                "{} · {snippet}",
                session.name.as_deref().unwrap_or_default()
            ))
            .disabled(!self.ready)
            .child(
                v_flex()
                    .min_w_0()
                    .w_full()
                    .child(
                        div()
                            .text_ellipsis()
                            .child(session.name.clone().unwrap_or_else(|| t!("ak.new"))),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .text_ellipsis()
                            .child(snippet.to_string()),
                    ),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.selected = Some(id.clone());
                this.focus.focus(window, cx);
                cx.emit(SidebarIntent::OpenMatch {
                    session_id: id.clone(),
                    query: query.clone(),
                });
            }))
            .into_any_element()
    }
}
