//! Native approval controls. Every decision requires an explicit user gesture.
mod notices;
mod render;
use crate::{WorkbenchIntent, t};
use agentkit_store::entities::SessionStore;
use gpui_kit::component::WindowExt;
use gpui_kit::component::input::{Enter, Escape};
use gpui_kit::{prelude::*, *};
pub(crate) use notices::render_notices;
gpui_kit::actions!(
    approval,
    [
        /// Explicitly allow the focused request once.
        ApprovePermission,
        /// Explicitly allow this tool for the current session when supported.
        AlwaysApprovePermission,
        /// Explicitly reject the focused request.
        RejectPermission
    ]
);
#[derive(Default)]
struct Bindings;
impl Global for Bindings {}
/// Focusable warning card for the first canonical pending request.
pub struct PermissionPrompt {
    store: Option<Entity<SessionStore>>,
    ready: bool,
    first: Option<String>,
    local_pending: bool,
    focus: FocusHandle,
    needs_focus: bool,
    previous: Option<FocusHandle>,
    subscription: Option<Subscription>,
}
impl EventEmitter<WorkbenchIntent> for PermissionPrompt {}
impl PermissionPrompt {
    /// Install approval-only keyboard bindings and create a hidden prompt.
    pub fn new(cx: &mut Context<Self>) -> Self {
        if cx.try_global::<Bindings>().is_none() {
            cx.set_global(Bindings);
            cx.bind_keys([
                KeyBinding::new("enter", ApprovePermission, Some("PermissionPrompt")),
                KeyBinding::new(
                    if cfg!(target_os = "macos") {
                        "cmd-enter"
                    } else {
                        "ctrl-enter"
                    },
                    AlwaysApprovePermission,
                    Some("PermissionPrompt"),
                ),
                KeyBinding::new("escape", RejectPermission, Some("PermissionPrompt")),
            ]);
        }
        Self {
            store: None,
            ready: false,
            first: None,
            local_pending: false,
            focus: cx.focus_handle(),
            needs_focus: false,
            previous: None,
            subscription: None,
        }
    }
    /// Attach a canonical session; subscriptions and focus remain window-local.
    pub fn bind(
        &mut self,
        store: Option<Entity<SessionStore>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.store.as_ref().map(Entity::entity_id) == store.as_ref().map(Entity::entity_id) {
            return;
        }
        if self.focus.contains_focused(window, cx)
            && let Some(previous) = self.previous.take()
        {
            previous.focus(window, cx);
        }
        self.previous = None;
        self.first = None;
        self.local_pending = false;
        self.subscription = None;
        self.store = store.clone();
        if let Some(store) = store {
            self.subscription =
                Some(cx.observe_in(&store, window, |this, _, window, cx| this.sync(window, cx)));
        }
        self.sync(window, cx);
    }
    /// Suspend a hidden prompt without dropping its canonical request queue.
    pub fn suspend(&mut self, cx: &mut Context<Self>) {
        self.ready = false;
        self.local_pending = false;
        self.needs_focus = false;
        cx.notify();
    }
    /// Gate actions while loading or disconnected without discarding request details.
    pub fn set_ready(&mut self, ready: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.ready != ready {
            self.ready = ready;
            if !ready {
                self.local_pending = false;
            } else {
                self.first = None;
            }
            self.sync(window, cx);
        }
    }
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let next = self.store.as_ref().and_then(|s| {
            s.read(cx)
                .model
                .permissions
                .first()
                .map(|r| r.request_id.clone())
        });
        if self.first != next {
            self.local_pending = false;
            self.needs_focus = next.is_some() && self.ready;
            if next.is_some()
                && self.ready
                && !window.has_active_dialog(cx)
                && !window.has_active_sheet(cx)
            {
                if self.previous.is_none() {
                    self.previous = window.focused(cx);
                }
                self.focus.focus(window, cx);
                self.needs_focus = false;
            } else if next.is_none() {
                let previous = self.previous.take();
                if self.focus.contains_focused(window, cx)
                    && let Some(focus) = previous
                {
                    focus.focus(window, cx);
                }
            }
            self.first = next;
        }
        if let Some(store) = &self.store
            && store
                .read(cx)
                .approval
                .error
                .as_ref()
                .is_some_and(|(id, _)| Some(id) == self.first.as_ref())
        {
            self.local_pending = false;
        }
        cx.notify();
    }
    fn respond(&mut self, allowed: bool, always: bool, cx: &mut Context<Self>) {
        let Some(store) = &self.store else {
            return;
        };
        let s = store.read(cx);
        let Some(request) = s.model.permissions.first() else {
            return;
        };
        if !self.ready
            || self.local_pending
            || s.approval.pending.is_some()
            || always && (!allowed || request.can_always_allow != Some(true))
        {
            return;
        }
        self.local_pending = true;
        cx.emit(WorkbenchIntent::Permission {
            session_id: s.model.state.session.id.clone(),
            request_id: request.request_id.clone(),
            allowed,
            always,
        });
        cx.notify();
    }
}
