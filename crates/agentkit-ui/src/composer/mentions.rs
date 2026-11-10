use super::*;
use agentkit_store::core::mentions::{self, Candidate, Kind};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::InlineToken,
    popover::Popover,
};
impl Composer {
    pub(crate) fn set_text(
        &mut self,
        text: &str,
        skills: &[agentkit_store::core::mentions::Skill],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use gpui_kit::component::input::InputContent;
        let mut content = InputContent::new(text.to_owned());
        for badge in mentions::metadata(text, skills).badges.iter().flatten() {
            if let Some(range) = mentions::byte_range(text, badge.start, badge.end) {
                let raw = &text[range.clone()];
                let token = InlineToken::new(format!("{}:{}", range.start, raw), raw.to_owned())
                    .with_label(badge.label.clone());
                if let Ok(next) = content.clone().with_token(range, token) {
                    content = next;
                }
            }
        }
        self.close_mentions(cx);
        self.input.update(cx, |input, cx| {
            input.set_value(content, window, cx);
            input.set_selected_range(text.len()..text.len(), cx);
        });
        if let Some(store) = &self.store {
            cx.emit(WorkbenchIntent::DraftChanged {
                session_id: store.read(cx).model.state.session.id.clone(),
                text: text.to_owned(),
            });
        }
    }
    pub(super) fn dismiss_mentions(&mut self, cx: &mut Context<Self>) {
        self.mention_dismissed = self.mention_query.clone();
        self.close_mentions(cx);
    }
    pub(super) fn close_mentions(&mut self, cx: &mut Context<Self>) {
        let open = self.mention_query.take().is_some();
        self.mention_task = None;
        self.mention_request += 1;
        if open {
            cx.notify();
        }
    }
    pub(super) fn refresh_mentions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selectable() || self.composing(window, cx) {
            self.close_mentions(cx);
            return;
        }
        let input = self.input.read(cx);
        let range = input.selected_range();
        let query = range
            .is_empty()
            .then(|| mentions::query(&input.value(), range.end))
            .flatten()
            .filter(|q| {
                !input
                    .tokens()
                    .iter()
                    .any(|span| span.range().contains(&q.range.start))
            });
        if query.is_some() && query == self.mention_dismissed {
            return;
        }
        self.mention_dismissed = None;
        if query == self.mention_query {
            return;
        }
        self.close_mentions(cx);
        let Some(query) = query else {
            return;
        };
        self.mention_query = Some(query.clone());
        if query.kind == Kind::Skill {
            self.refresh_skills(window, cx);
            return;
        }
        self.mention_menu.update(cx, |menu, cx| {
            menu.set(Vec::new(), t!("ak.loading"), window, cx)
        });
        let Some(session_id) = self.session(cx).map(|s| s.id.clone()) else {
            return;
        };
        let request = self.mention_request;
        self.mention_task = Some(cx.spawn_in(window, async move |weak, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(150))
                .await;
            let _ = weak.update_in(cx, |this, _, cx| {
                if this.mention_request == request && this.mention_query.as_ref() == Some(&query) {
                    cx.emit(WorkbenchIntent::SearchMentions {
                        session_id,
                        request,
                        query: query.text,
                    });
                }
            });
        }));
        cx.notify();
    }
    pub(super) fn refresh_skills(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(query) = &self.mention_query else {
            return;
        };
        if let Some(session) = self.session(cx) {
            cx.emit(WorkbenchIntent::LoadMentionSkills(session.id.clone()));
        }
        let mut items = mentions::skill_candidates(&self.mention_skills, &query.text);
        if "compact".starts_with(&query.text.to_lowercase()) {
            items.push(Candidate {
                label: "/compact".into(),
                detail: t!("ak.mention.compact"),
                raw: "/compact".into(),
                group: "command",
            });
        }
        let status = if items.is_empty() {
            t!("ak.mention.empty")
        } else {
            String::new()
        };
        self.mention_menu
            .update(cx, |menu, cx| menu.set(items, status, window, cx));
        cx.notify();
    }
    pub(super) fn accept_mention(
        &mut self,
        item: Candidate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.selectable() || self.composing(window, cx) {
            return;
        }
        let Some(query) = self.mention_query.clone() else {
            return;
        };
        let input = self.input.read(cx);
        let cursor = input.selected_range();
        if !cursor.is_empty()
            || mentions::query(&input.value(), cursor.end).as_ref() != Some(&query)
        {
            self.close_mentions(cx);
            return;
        }
        self.close_mentions(cx);
        if item.group == "retry" {
            self.refresh_mentions(window, cx);
            return;
        }
        self.input.update(cx, |input, cx| {
            let token = InlineToken::new(item.raw.clone(), item.raw).with_label(item.label);
            let _ = input.replace_range_with_token(query.range, token, window, cx);
            input.focus_handle(cx).focus(window, cx);
        });
    }
    pub(super) fn mention_results(
        &mut self,
        session: &str,
        request: u64,
        query: &str,
        result: Result<Vec<g::FileSearchResult>, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.ready
            || self.session(cx).is_none_or(|s| s.id != session)
            || self.mention_request != request
            || self
                .mention_query
                .as_ref()
                .is_none_or(|q| q.kind != Kind::File || q.text != query)
        {
            return;
        }
        let (mut items, status) = match result {
            Ok(files) => {
                let items = files
                    .into_iter()
                    .filter_map(|f| {
                        mentions::file_candidate(
                            &f.relative_path,
                            f.type_ == g::FileSearchResultType::Directory,
                        )
                    })
                    .collect::<Vec<_>>();
                let status = if items.is_empty() {
                    t!("ak.mention.empty")
                } else {
                    String::new()
                };
                (items, status)
            }
            Err(e) => (
                vec![Candidate {
                    label: t!("ak.retry"),
                    detail: String::new(),
                    raw: String::new(),
                    group: "retry",
                }],
                t!("ak.mention.failed", error = e.as_str()),
            ),
        };
        items.sort_by_key(|item| (item.group, item.detail.clone()));
        self.mention_menu
            .update(cx, |menu, cx| menu.set(items, status, window, cx));
        cx.notify();
    }
    pub(super) fn mention_popover(&self, cx: &Context<Self>) -> impl IntoElement {
        let menu = self.mention_menu.clone();
        Popover::new("mentions")
            .anchor(Anchor::BottomLeft)
            .open(self.mention_query.is_some())
            .track_focus(&self.input.read(cx).focus_handle(cx))
            .trigger(
                Button::new("mention-anchor")
                    .ghost()
                    .accessibility_label(t!("ak.mention.skills"))
                    .tab_stop(false)
                    .opacity(0.)
                    .w_full()
                    .h(px(1.)),
            )
            .content(move |_, _, _| menu.clone())
            .on_open_change(cx.listener(|this, open: &bool, _, cx| {
                if !*open {
                    this.dismiss_mentions(cx);
                }
            }))
    }
    pub(super) fn mention_key(
        &mut self,
        delta: Option<isize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.refresh_mentions(window, cx);
        if self.mention_query.is_none() || !self.input.read(cx).focus_handle(cx).is_focused(window)
        {
            return false;
        }
        if self.composing(window, cx) {
            return true;
        }
        if let Some(delta) = delta {
            self.mention_menu
                .update(cx, |m, cx| m.move_by(delta, window, cx));
        } else if let Some(item) = self.mention_menu.read(cx).pick(cx) {
            self.accept_mention(item, window, cx);
        }
        true
    }
    pub(super) fn mention_left(
        &mut self,
        _: &gpui_kit::component::input::MoveLeft,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dismiss_mentions(cx);
        cx.propagate();
    }
    pub(super) fn mention_right(
        &mut self,
        _: &gpui_kit::component::input::MoveRight,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dismiss_mentions(cx);
        cx.propagate();
    }
    pub(super) fn mention_down(
        &mut self,
        _: &gpui_kit::component::input::MoveDown,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mention_key(Some(1), window, cx) {
            cx.stop_propagation();
        } else {
            cx.propagate();
        }
    }
    pub(super) fn mention_tab(
        &mut self,
        _: &gpui_kit::component::input::IndentInline,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mention_key(None, window, cx) {
            cx.stop_propagation();
        } else {
            cx.propagate();
        }
    }
}
impl WorkbenchView {
    /// Complete only the current session's latest mention search.
    pub fn mention_results(
        &mut self,
        session: &str,
        request: u64,
        query: &str,
        result: Result<Vec<g::FileSearchResult>, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.composer.update(cx, |c, cx| {
            c.mention_results(session, request, query, result, window, cx)
        });
    }
}
use crate::WorkbenchView;
