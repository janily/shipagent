//! Native workspace and session navigation; IO belongs to the application controller.
mod dialog;
mod keyboard;
/// Scoped sidebar keyboard actions, shared with the application keymap.
pub use keyboard::{Delete as DeleteAction, Rename as RenameAction};
mod render;
mod row;
mod search;
mod views;
mod workspace;
use crate::t;
use agentkit_protocol::generated as g;
use agentkit_store::core::views::{Mutation, View};
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
    /// Debounced body search owned by the controller; query changes invalidate old results.
    Search {
        /// Window-local query generation.
        request: u64,
        /// Trimmed user input; empty cancels the debounce.
        query: String,
    },
    /// Open a body match and locate the matching message after hydration.
    OpenMatch {
        /// Canonical session identity.
        session_id: String,
        /// Literal content to locate after opening.
        query: String,
    },
    /// Apply an explicit session state through the backend.
    Command {
        /// Canonical session identity.
        session_id: String,
        /// Desired authoritative state.
        mutation: Mutation,
    },
    /// Retry the background pending-plan projection.
    RetryPlans,
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
    rows: Vec<ListRow>,
    view: View,
    workspace_id: String,
    workspace_subscription: Option<Subscription>,
    search_request: u64,
    search_results: Vec<g::SessionSearchResult>,
    searching: bool,
    search_error: bool,
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
                this.query_changed(cx);
            }
        });
        Self {
            list: None,
            workspace,
            search,
            rows: vec![],
            view: View::All,
            workspace_id: String::new(),
            workspace_subscription: None,
            search_request: 0,
            search_results: vec![],
            searching: false,
            search_error: false,
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
    /// Focus search or the session list without changing selection.
    pub fn focus_region(&self, search: bool, window: &mut Window, cx: &mut Context<Self>) {
        if search {
            self.search.read(cx).focus_handle(cx).focus(window, cx);
        } else {
            self.focus.focus(window, cx);
        }
    }
    /// Whether keyboard focus currently belongs to this navigation region.
    pub fn focused(&self, window: &Window, cx: &App) -> bool {
        self.focus.contains_focused(window, cx)
    }
    /// Refresh cached input copy after a live locale change.
    pub fn localize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.update(cx, |s, cx| {
            s.set_placeholder(t!("ak.nav.search"), window, cx)
        });
        self.workspace.update(cx, |_, cx| cx.notify());
        cx.notify();
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
        let changed = self.workspace_id != store.read(cx).id;
        self.workspace_id = store.read(cx).id.clone();
        self.workspace_subscription = Some(cx.observe(&store, |this, store, cx| {
            let id = store.read(cx).id.clone();
            if this.workspace_id != id {
                this.workspace_id = id;
                this.view = View::All;
                this.query_changed(cx);
            }
        }));
        self.workspace.update(cx, |w, cx| w.bind(store, cx));
        if changed {
            self.view = View::All;
            self.query_changed(cx);
        } else {
            self.rebuild(cx);
        }
    }
    /// Set connection/loading gates without losing local search or keyboard selection.
    pub fn set_context(&mut self, active: Option<String>, ready: bool, cx: &mut Context<Self>) {
        let changed = self.active != active || self.ready != ready;
        if self.active != active {
            self.active = active.clone();
            self.selected = active;
        }
        let was_ready = self.ready;
        self.ready = ready;
        if was_ready != ready {
            self.query_changed(cx);
        }
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
        let query = self.search.read(cx).value().trim().to_string();
        self.rows.clear();
        if let Some(list) = &self.list {
            let list = list.read(cx);
            let sessions = &list.model.sessions;
            let workspace = (!self.workspace_id.is_empty()).then_some(self.workspace_id.as_str());
            let titles = agentkit_store::core::views::indices(
                sessions,
                &list.model.plans,
                workspace,
                self.view,
                &query,
            );
            self.rows
                .extend(titles.iter().map(|i| ListRow::Session(*i)));
            if !query.is_empty() {
                let candidates = agentkit_store::core::views::indices(
                    sessions,
                    &list.model.plans,
                    workspace,
                    self.view,
                    "",
                );
                let content: Vec<_> = candidates
                    .into_iter()
                    .filter(|i| !titles.contains(i))
                    .filter_map(|i| {
                        let hit = self
                            .search_results
                            .iter()
                            .find(|r| r.session_id == sessions[i].id)?;
                        let snippet = hit
                            .matches
                            .first()
                            .map(|m| m.snippet.clone())
                            .unwrap_or_else(|| t!("ak.nav.snippet_missing"));
                        Some(ListRow::Content(i, snippet))
                    })
                    .collect();
                if !content.is_empty() {
                    self.rows.push(ListRow::Header);
                    self.rows.extend(content);
                }
            }
        }
        self.sizes = Rc::new(
            self.rows
                .iter()
                .map(|r| {
                    size(
                        px(240.),
                        px(if matches!(r, ListRow::Header) {
                            24.
                        } else {
                            52.
                        }),
                    )
                })
                .collect(),
        );
        if self.selected.as_ref().is_none_or(|id| {
            !self
                .rows
                .iter()
                .any(|r| self.row_id(r, cx).as_ref() == Some(id))
        }) {
            self.selected = self.rows.iter().find_map(|r| self.row_id(r, cx));
        }
        cx.notify();
    }
    fn row_id(&self, row: &ListRow, cx: &App) -> Option<String> {
        let index = row.index()?;
        self.list
            .as_ref()?
            .read(cx)
            .model
            .sessions
            .get(index)
            .map(|s| s.id.clone())
    }
}
#[derive(Clone)]
enum ListRow {
    Session(usize),
    Content(usize, String),
    Header,
}
impl ListRow {
    fn index(&self) -> Option<usize> {
        match self {
            Self::Session(i) | Self::Content(i, _) => Some(*i),
            Self::Header => None,
        }
    }
}
