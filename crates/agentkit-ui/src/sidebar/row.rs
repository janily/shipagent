use super::*;
use agentkit_protocol::generated::Session;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    menu::{ContextMenuExt, DropdownMenu, PopupMenu},
    spinner::Spinner,
    *,
};
use serde::Deserialize;
#[derive(Clone, PartialEq, Deserialize, Action)]
#[action(namespace=navigation,no_json)]
pub(super) struct RenameSession {
    pub id: String,
}
#[derive(Clone, PartialEq, Deserialize, Action)]
#[action(namespace=navigation,no_json)]
pub(super) struct DeleteSession {
    pub id: String,
}
#[derive(Clone, PartialEq, Deserialize, Action)]
#[action(namespace=navigation,no_json)]
pub(super) struct MutateSession {
    pub id: String,
    pub mutation: Mutation,
}
fn commands(
    mut menu: PopupMenu,
    id: &str,
    flagged: bool,
    unread: bool,
    archived: bool,
) -> PopupMenu {
    let choices = [
        if flagged {
            (t!("ak.nav.unflag"), Mutation::Unflag)
        } else {
            (t!("ak.nav.flag"), Mutation::Flag)
        },
        if unread {
            (t!("ak.nav.mark_read"), Mutation::MarkRead)
        } else {
            (t!("ak.nav.mark_unread"), Mutation::MarkUnread)
        },
        if archived {
            (t!("ak.nav.unarchive"), Mutation::Unarchive)
        } else {
            (t!("ak.nav.archive"), Mutation::Archive)
        },
    ];
    for (label, mutation) in choices {
        menu = menu.menu(
            label,
            Box::new(MutateSession {
                id: id.into(),
                mutation,
            }),
        );
    }
    menu
}
impl SessionSidebar {
    pub(super) fn row(
        &self,
        s: &Session,
        pending: bool,
        plan: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let flagged = s.is_flagged == Some(true);
        let unread = s.has_unread == Some(true);
        let archived = s.is_archived == Some(true);
        let id = s.id.clone();
        let menu_id = id.clone();
        let more_id = id.clone();
        let action_focus = self.focus.clone();
        let context_focus = self.focus.clone();
        let name = s
            .name
            .clone()
            .or(s.preview.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| t!("ak.new"));
        let status = if pending {
            t!("ak.permission_wait")
        } else if s.is_processing {
            t!("ak.nav.running")
        } else if plan {
            t!("ak.nav.plan_wait")
        } else if s.has_unread == Some(true) {
            t!("ak.nav.unread")
        } else {
            relative_time(s.last_message_at)
        };
        let snippet = self
            .search_results
            .iter()
            .find(|r| r.session_id == s.id)
            .map(|r| {
                r.matches
                    .first()
                    .map(|m| m.snippet.clone())
                    .unwrap_or_else(|| t!("ak.nav.snippet_missing"))
            });
        let content_hit = snippet.is_some();
        let query = self.search.read(cx).value().trim().to_string();
        let active = self.active.as_ref() == Some(&id);
        let selected = self.selected.as_ref() == Some(&id);
        let button = Button::new(SharedString::from(format!("session:{id}")))
            .ghost()
            .h_8()
            .w_full()
            .min_w_0()
            .justify_start()
            .px_2()
            .child(div().w_full().min_w_0().text_ellipsis().child(name.clone()))
            .accessibility_label(format!(
                "{name} · {}",
                snippet.as_deref().unwrap_or(&status)
            ))
            .disabled(!self.ready)
            .when(active || selected, |b| b.bg(cx.theme().sidebar_accent))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.selected = Some(id.clone());
                this.focus.focus(window, cx);
                if content_hit {
                    cx.emit(SidebarIntent::OpenMatch {
                        session_id: id.clone(),
                        query: query.clone(),
                    });
                } else {
                    cx.emit(SidebarIntent::Open(id.clone()));
                }
            }));
        h_flex()
            .h(px(52.))
            .w_full()
            .gap_1()
            .items_center()
            .child(
                div()
                    .w_3()
                    .flex_shrink_0()
                    .when(pending && !active, |v| {
                        v.child(
                            div()
                                .id(SharedString::from(format!("approval-dot:{}", s.id)))
                                .size_2()
                                .rounded_full()
                                .bg(cx.theme().warning)
                                .test_support(),
                        )
                    })
                    .when(!pending && s.is_processing, |v| {
                        v.child(Spinner::new().small())
                    })
                    .when(
                        !pending && !s.is_processing && (plan || s.has_unread == Some(true)),
                        |v| v.child(div().size_2().rounded_full().bg(cx.theme().primary)),
                    ),
            )
            .when(s.is_flagged == Some(true), |v| {
                v.child(
                    div()
                        .id(SharedString::from(format!("flag:{}", s.id)))
                        .text_sm()
                        .aria_label(t!("ak.nav.flagged"))
                        .text_color(cx.theme().muted_foreground)
                        .child("⚑"),
                )
            })
            .child(
                v_flex().flex_1().min_w_0().child(button).child(
                    div()
                        .h_5()
                        .px_2()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(snippet.unwrap_or(status)),
                ),
            )
            .child(
                Button::new(SharedString::from(format!("session-menu:{}", s.id)))
                    .ghost()
                    .small()
                    .label("…")
                    .accessibility_label(t!("ak.nav.actions"))
                    .disabled(!self.ready)
                    .dropdown_menu_with_anchor(Anchor::TopRight, move |m, _, _| {
                        commands(
                            m.action_context(action_focus.clone()),
                            &more_id,
                            flagged,
                            unread,
                            archived,
                        )
                        .menu(
                            t!("ak.nav.rename"),
                            Box::new(RenameSession {
                                id: more_id.clone(),
                            }),
                        )
                        .separator()
                        .menu_element(
                            Box::new(DeleteSession {
                                id: more_id.clone(),
                            }),
                            |_, cx| {
                                div()
                                    .text_color(cx.theme().danger)
                                    .child(t!("ak.nav.delete"))
                            },
                        )
                    }),
            )
            .context_menu(move |m, _, _| {
                commands(
                    m.action_context(context_focus.clone()),
                    &menu_id,
                    flagged,
                    unread,
                    archived,
                )
                .menu(
                    t!("ak.nav.rename"),
                    Box::new(RenameSession {
                        id: menu_id.clone(),
                    }),
                )
                .separator()
                .menu_element(
                    Box::new(DeleteSession {
                        id: menu_id.clone(),
                    }),
                    |_, cx| {
                        div()
                            .text_color(cx.theme().danger)
                            .child(t!("ak.nav.delete"))
                    },
                )
            })
            .into_any_element()
    }
}
fn relative_time(timestamp: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64);
    let mins = now.saturating_sub(timestamp).max(0) / 60000;
    if mins < 1 {
        t!("ak.nav.just_now")
    } else if mins < 60 {
        t!("ak.nav.minutes", count = mins)
    } else if mins < 1440 {
        t!("ak.nav.hours", count = mins / 60)
    } else {
        t!("ak.nav.days", count = mins / 1440)
    }
}
