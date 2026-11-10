//! Native Kit list; the editor retains focus while arrows select candidates.
use super::*;
use agentkit_store::core::mentions::Candidate;
use gpui_kit::component::{
    ActiveTheme, IndexPath,
    list::{List, ListDelegate, ListEvent, ListItem, ListState},
    v_flex,
};
struct Rows {
    items: Vec<Candidate>,
    selected: Option<IndexPath>,
}
impl Rows {
    fn groups(&self) -> Vec<&'static str> {
        let mut groups = Vec::new();
        for item in &self.items {
            if !groups.contains(&item.group) {
                groups.push(item.group);
            }
        }
        groups
    }
    fn item(&self, ix: IndexPath) -> Option<&Candidate> {
        let group = *self.groups().get(ix.section)?;
        self.items.iter().filter(|i| i.group == group).nth(ix.row)
    }
    fn indices(&self) -> Vec<IndexPath> {
        self.groups()
            .into_iter()
            .enumerate()
            .flat_map(|(section, group)| {
                (0..self.items.iter().filter(|i| i.group == group).count()).map(move |row| {
                    IndexPath {
                        section,
                        row,
                        ..Default::default()
                    }
                })
            })
            .collect()
    }
}
impl ListDelegate for Rows {
    type Item = ListItem;
    fn sections_count(&self, _: &App) -> usize {
        self.groups().len().max(1)
    }
    fn items_count(&self, section: usize, _: &App) -> usize {
        self.groups()
            .get(section)
            .map(|g| self.items.iter().filter(|i| i.group == *g).count())
            .unwrap_or(0)
    }
    fn render_section_header(
        &mut self,
        section: usize,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<impl IntoElement> {
        let group = match *self.groups().get(section)? {
            "folder" => t!("ak.mention.folders"),
            "file" => t!("ak.mention.files"),
            "command" => t!("ak.mention.commands"),
            _ => t!("ak.mention.skills"),
        };
        Some(
            div()
                .px_2()
                .h(px(24.))
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(group),
        )
    }
    fn render_item(
        &mut self,
        ix: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<ListItem> {
        let item = self.item(ix)?;
        Some(
            ListItem::new(("mention-option", ix.section * 10000 + ix.row))
                .accessibility_label(format!("{} {}", item.label, item.detail))
                .selected(self.selected == Some(ix))
                .child(
                    v_flex()
                        .h(px(48.))
                        .justify_center()
                        .child(item.label.clone())
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(item.detail.clone()),
                        ),
                ),
        )
    }
    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
        self.selected = ix;
    }
}
pub(super) struct MentionMenu {
    list: Entity<ListState<Rows>>,
    items: Vec<Candidate>,
    pub status: String,
    _subscription: Subscription,
}
impl EventEmitter<Candidate> for MentionMenu {}
impl MentionMenu {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let list = cx.new(|cx| {
            ListState::new(
                Rows {
                    items: Vec::new(),
                    selected: None,
                },
                window,
                cx,
            )
            .searchable(false)
        });
        let sub = cx.subscribe(&list, |this, _, e, cx| {
            if let ListEvent::Confirm(ix) = e
                && let Some(item) = this.list.read(cx).delegate().item(*ix)
            {
                cx.emit(item.clone());
            }
        });
        Self {
            list,
            items: Vec::new(),
            status: String::new(),
            _subscription: sub,
        }
    }
    pub fn set(
        &mut self,
        items: Vec<Candidate>,
        status: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.items = items.clone();
        self.status = status;
        self.list.update(cx, |list, cx| {
            list.delegate_mut().items = items;
            list.set_selected_index(
                (!self.items.is_empty()).then_some(IndexPath::new(0)),
                window,
                cx,
            );
            cx.notify();
        });
        cx.notify();
    }
    pub fn move_by(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.items.len();
        if count == 0 {
            return;
        }
        self.list.update(cx, |list, cx| {
            let indices = list.delegate().indices();
            let index = list
                .selected_index()
                .and_then(|i| indices.iter().position(|p| *p == i))
                .unwrap_or(0);
            let row = (index as isize + delta).rem_euclid(count as isize) as usize;
            list.set_selected_index(Some(indices[row]), window, cx);
            list.scroll_to_selected_item(window, cx);
            cx.notify();
        });
    }
    pub fn pick(&self, cx: &App) -> Option<Candidate> {
        let list = self.list.read(cx);
        list.delegate().item(list.selected_index()?).cloned()
    }
}
impl Render for MentionMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("mention-menu")
            .test_support()
            .w(px(440.))
            .max_h(px(300.))
            .gap_1()
            .when(!self.status.is_empty(), |v| {
                v.child(
                    div()
                        .p_2()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(self.status.clone()),
                )
            })
            .when(!self.items.is_empty(), |v| {
                v.child(List::new(&self.list).max_h(px(260.)))
            })
    }
}
