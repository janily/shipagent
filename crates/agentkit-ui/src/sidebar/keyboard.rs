use super::*;
gpui_kit::actions!(
    navigation,
    [
        Up,
        Down,
        First,
        Last,
        Open,
        /// Rename the selected session after explicit input.
        Rename,
        /// Confirm deletion of the selected session.
        Delete,
        Clear
    ]
);
#[derive(Default)]
struct Bound;
impl Global for Bound {}
pub(super) fn bindings(cx: &mut App) {
    if cx.try_global::<Bound>().is_some() {
        return;
    }
    cx.set_global(Bound);
    cx.bind_keys([
        KeyBinding::new("up", Up, Some("SessionSidebar")),
        KeyBinding::new("down", Down, Some("SessionSidebar")),
        KeyBinding::new("home", First, Some("SessionSidebar")),
        KeyBinding::new("end", Last, Some("SessionSidebar")),
        KeyBinding::new("enter", Open, Some("SessionSidebar")),
        KeyBinding::new("f2", Rename, Some("SessionSidebar")),
        KeyBinding::new("delete", Delete, Some("SessionSidebar")),
        KeyBinding::new("backspace", Delete, Some("SessionSidebar")),
        KeyBinding::new("escape", Clear, Some("SessionSearch")),
    ]);
}
impl SessionSidebar {
    pub(super) fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.rows.is_empty() {
            return;
        }
        let old = self
            .rows
            .iter()
            .position(|i| {
                self.list.as_ref().is_some_and(|s| {
                    Some(&s.read(cx).model.sessions[*i].id) == self.selected.as_ref()
                })
            })
            .unwrap_or(0);
        let ix = (old as isize)
            .saturating_add(delta)
            .clamp(0, self.rows.len() as isize - 1) as usize;
        self.selected = self
            .list
            .as_ref()
            .map(|s| s.read(cx).model.sessions[self.rows[ix]].id.clone());
        self.scroll.scroll_to_item(ix, ScrollStrategy::Top);
        cx.notify();
    }
    pub(super) fn selected_intent(&self, f: fn(String) -> SidebarIntent, cx: &mut Context<Self>) {
        if self.ready
            && let Some(id) = &self.selected
        {
            cx.emit(f(id.clone()));
        }
    }
}
