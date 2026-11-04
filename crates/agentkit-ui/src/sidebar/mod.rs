//! Native workspace and session navigation; IO belongs to the application controller.
mod dialog;
mod keyboard;
mod render;
mod row;
mod workspace;
use crate::t;
use agentkit_store::entities::{SessionListStore, WorkspaceStore};
pub use dialog::{NavigationDialog, NavigationForm};
use gpui_kit::component::{
    VirtualListScrollHandle,
    input::{InputEvent, InputState},
};
use gpui_kit::{prelude::*, *};
use std::rc::Rc;
/// Navigation gestures captured against stable session/workspace identities.
#[derive(Clone, Debug)]
pub enum SidebarIntent {
    /// Open one canonical session.
    Open(String),
    /// Create a session in the selected workspace.
    New,
    /// Ask for a new title.
    Rename(String),
    /// Ask for confirmation before deleting.
    Delete(String),
    /// Switch the window binding.
    SwitchWorkspace(String),
    /// Ask for name and local directory.
    CreateWorkspace,
    /// Ask for confirmation before unregistering.
    RemoveWorkspace(String),
}
/// Virtualized window-local conversation navigation.
pub struct SessionSidebar {
    list: Option<Entity<SessionListStore>>,
    workspace: Entity<workspace::WorkspaceSwitcher>,
    search: Entity<InputState>,
    rows: Vec<usize>,
    sizes: Rc<Vec<Size<Pixels>>>,
    active: Option<String>,
    selected: Option<String>,
    ready: bool,
    focus: FocusHandle,
    scroll: VirtualListScrollHandle,
    subscription: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<SidebarIntent> for SessionSidebar {}
impl SessionSidebar {
    /// Create controls without a server binding.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        keyboard::bindings(cx);
        let workspace = cx.new(workspace::WorkspaceSwitcher::new);
        let sub = cx.subscribe(&workspace, |_, _, intent: &SidebarIntent, cx| {
            cx.emit(intent.clone())
        });
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(t!("ak.nav.search")));
        let input = cx.subscribe(&search, |this, _, e, cx| {
            if matches!(e, InputEvent::Change) {
                this.rebuild(cx);
            }
        });
        Self {
            list: None,
            workspace,
            search,
            rows: vec![],
            sizes: Rc::new(vec![]),
            active: None,
            selected: None,
            ready: false,
            focus: cx.focus_handle(),
            scroll: VirtualListScrollHandle::new(),
            subscription: None,
            _subscriptions: vec![sub, input],
        }
    }
    /// Whether keyboard focus currently belongs to this navigation region.
    pub fn focused(&self, window: &Window, cx: &App) -> bool {
        self.focus.contains_focused(window, cx)
    }
    /// Attach the canonical list once; rebuild metadata only when summaries change.
    pub fn bind_list(&mut self, list: Entity<SessionListStore>, cx: &mut Context<Self>) {
        if self
            .list
            .as_ref()
            .is_some_and(|s| s.entity_id() == list.entity_id())
        {
            return;
        }
        self.subscription = Some(cx.observe(&list, |this, _, cx| this.rebuild(cx)));
        self.list = Some(list);
        self.rebuild(cx);
    }
    /// Attach the window workspace catalog.
    pub fn bind_workspace(&mut self, store: Entity<WorkspaceStore>, cx: &mut Context<Self>) {
        self.workspace.update(cx, |w, cx| w.bind(store, cx));
    }
    /// Set connection/loading gates without losing local search or keyboard selection.
    pub fn set_context(&mut self, active: Option<String>, ready: bool, cx: &mut Context<Self>) {
        let changed = self.active != active || self.ready != ready;
        if self.active != active {
            self.active = active.clone();
            self.selected = active;
        }
        self.ready = ready;
        self.workspace.update(cx, |w, cx| {
            if w.ready != ready {
                w.ready = ready;
                cx.notify();
            }
        });
        if changed {
            cx.notify();
        }
    }
    fn rebuild(&mut self, cx: &mut Context<Self>) {
        let query = self.search.read(cx).value().trim().to_lowercase();
        self.rows = self
            .list
            .as_ref()
            .map(|list| {
                let sessions = &list.read(cx).model.sessions;
                let mut rows: Vec<_> = sessions
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| {
                        s.hidden != Some(true)
                            && s.is_archived != Some(true)
                            && (query.is_empty()
                                || s.name
                                    .as_deref()
                                    .unwrap_or("")
                                    .to_lowercase()
                                    .contains(&query))
                    })
                    .map(|(i, _)| i)
                    .collect();
                rows.sort_by(|a, b| {
                    sessions[*b]
                        .last_message_at
                        .cmp(&sessions[*a].last_message_at)
                        .then_with(|| sessions[*a].id.cmp(&sessions[*b].id))
                });
                rows
            })
            .unwrap_or_default();
        self.sizes = Rc::new(
            (0..self.rows.len())
                .map(|_| size(px(240.), px(52.)))
                .collect(),
        );
        if self.selected.as_ref().is_none_or(|id| {
            !self.rows.iter().any(|i| {
                self.list
                    .as_ref()
                    .is_some_and(|s| s.read(cx).model.sessions[*i].id == *id)
            })
        }) {
            self.selected = self.rows.first().and_then(|i| {
                self.list
                    .as_ref()
                    .map(|s| s.read(cx).model.sessions[*i].id.clone())
            });
        }
        cx.notify();
    }
}
