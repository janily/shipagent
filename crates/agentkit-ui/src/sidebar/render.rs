use super::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::Input,
    v_virtual_list, *,
};
use keyboard::*;
const LIST_ID: &str = "session-list";
impl Render for SessionSidebar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("session-sidebar")
            .test_support()
            .track_focus(&self.focus)
            .key_context("SessionSidebar")
            .role(Role::Navigation)
            .aria_label(t!("ak.sessions"))
            .size_full()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().border)
            .p_3()
            .gap_2()
            .on_action(cx.listener(|this, _: &Up, _, cx| this.move_selection(-1, cx)))
            .on_action(cx.listener(|this, _: &Down, _, cx| this.move_selection(1, cx)))
            .on_action(cx.listener(|this, _: &First, _, cx| this.move_selection(isize::MIN, cx)))
            .on_action(cx.listener(|this, _: &Last, _, cx| this.move_selection(isize::MAX, cx)))
            .on_action(
                cx.listener(|this, _: &Open, _, cx| this.selected_intent(SidebarIntent::Open, cx)),
            )
            .on_action(cx.listener(|this, _: &Rename, _, cx| {
                this.selected_intent(SidebarIntent::Rename, cx)
            }))
            .on_action(cx.listener(|this, _: &Delete, _, cx| {
                this.selected_intent(SidebarIntent::Delete, cx)
            }))
            .on_action(cx.listener(|this, a: &row::RenameSession, _, cx| {
                if this.ready {
                    cx.emit(SidebarIntent::Rename(a.id.clone()));
                }
            }))
            .on_action(cx.listener(|this, a: &row::DeleteSession, _, cx| {
                if this.ready {
                    cx.emit(SidebarIntent::Delete(a.id.clone()));
                }
            }))
            .child(self.workspace.clone())
            .child(
                Button::new("new-session")
                    .primary()
                    .w_full()
                    .label(t!("ak.new"))
                    .disabled(!self.ready)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(SidebarIntent::New))),
            )
            .child(
                div()
                    .key_context("SessionSearch")
                    .on_action(cx.listener(|this, _: &Clear, window, cx| {
                        this.search.update(cx, |s, cx| s.set_value("", window, cx));
                    }))
                    .child(
                        Input::new(&self.search)
                            .id("session-search")
                            .disabled(!self.ready),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("ak.sessions")),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .when(self.rows.is_empty(), |v| {
                        v.child(
                            div()
                                .p_3()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(if self.search.read(cx).value().is_empty() {
                                    t!("ak.nav.empty")
                                } else {
                                    t!("ak.nav.no_results")
                                }),
                        )
                    })
                    .when(!self.rows.is_empty(), |v| {
                        v.child(
                            v_virtual_list(
                                cx.entity(),
                                LIST_ID,
                                self.sizes.clone(),
                                |this, range, _, cx| {
                                    range
                                        .filter_map(|i| {
                                            let s = this.list.as_ref()?.read(cx);
                                            let session =
                                                s.model.sessions.get(*this.rows.get(i)?)?.clone();
                                            let pending = s
                                                .model
                                                .awaiting
                                                .get(&session.id)
                                                .is_some_and(|r| !r.is_empty());
                                            let plan = s.model.plans.contains(&session.id);
                                            Some(this.row(&session, pending, plan, cx))
                                        })
                                        .collect()
                                },
                            )
                            .track_scroll(&self.scroll)
                            .size_full(),
                        )
                    }),
            )
    }
}
