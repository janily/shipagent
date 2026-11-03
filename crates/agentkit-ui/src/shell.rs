#[path = "shell_center.rs"]
mod center;
#[path = "shell_context.rs"]
mod context_panel;
#[path = "shell_frame.rs"]
mod frame;
#[path = "shell_sidebar.rs"]
mod sidebar;
use super::*;
use crate::layout::PanelTab;
use gpui_kit::component::{
    resizable::{h_resizable, resizable_panel},
    skeleton::Skeleton,
    status_bar::StatusBar,
};

impl WorkbenchView {
    /// Show that local layout preferences can no longer be saved.
    pub fn set_preferences_writable(&mut self, writable: bool, cx: &mut Context<Self>) {
        self.preferences_writable = writable;
        cx.notify();
    }
    /// Apply validated stored layout without writing responsive collapse back to disk.
    pub fn apply_shell(
        &mut self,
        mut shell: ShellPreferences,
        writable: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        shell.validate();
        self.shell = shell;
        self.panel_sizes.update(cx, |s, cx| {
            s.clear();
            cx.notify();
        });
        self.preferences_writable = writable;
        self.sync_sheet(window, cx);
        cx.notify();
    }
    /// Toggle sidebar; narrow viewports expose it as a keyboard-accessible sheet.
    pub fn toggle_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if f32::from(window.bounds().size.width) < 1100. {
            self.sidebar_sheet = !self.sidebar_sheet;
        } else {
            self.shell.sidebar_open = !self.shell.sidebar_open;
            self.persist_shell(cx);
        }
        self.sync_sheet(window, cx);
        cx.notify();
    }
    /// Toggle the context region, choosing Sheet or inline presentation by viewport.
    pub fn toggle_right_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_sheet = false;
        self.shell.right_open = !self.shell.right_open;
        self.persist_shell(cx);
        self.sync_sheet(window, cx);
        cx.notify();
    }
    /// Temporarily hide side regions while retaining the user's expanded widths.
    pub fn toggle_focus_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_sheet = false;
        self.shell.focus_mode = !self.shell.focus_mode;
        self.persist_shell(cx);
        self.sync_sheet(window, cx);
        cx.notify();
    }
    /// Reconcile adaptive presentation after a native window resize.
    pub fn viewport_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_sheet(window, cx);
        cx.notify();
    }
    fn persist_shell(&self, cx: &mut Context<Self>) {
        cx.emit(WorkbenchIntent::LayoutChanged(self.shell.clone()));
    }
    fn sync_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let width = f32::from(window.bounds().size.width);
        if width >= 1100. {
            self.sidebar_sheet = false;
        }
        let kind = if self.sidebar_sheet && !self.shell.focus_mode {
            Some(true)
        } else if self.shell.right_is_sheet(width) {
            Some(false)
        } else {
            None
        };
        if kind == self.sheet_kind {
            return;
        }
        if self.sheet_kind.is_some() {
            window.close_sheet(cx);
        }
        self.sheet_kind = kind;
        if let Some(sidebar) = kind {
            let weak = cx.weak_entity();
            let overlay = if let Some(overlay) = &self.overlay {
                overlay.update(cx, |v, cx| {
                    v.sidebar = sidebar;
                    cx.notify();
                });
                overlay.clone()
            } else {
                let owner = cx.entity();
                let overlay = cx.new(|cx| {
                    let observation = cx.observe(&owner, |_, _, cx| cx.notify());
                    ShellOverlay {
                        owner: owner.downgrade(),
                        sidebar,
                        _observation: observation,
                    }
                });
                self.overlay = Some(overlay.clone());
                overlay
            };
            let width = if sidebar {
                self.shell.sidebar_width
            } else {
                self.shell.right_width
            };
            window.open_sheet_at(
                if sidebar {
                    Placement::Left
                } else {
                    Placement::Right
                },
                cx,
                move |sheet, _, _| {
                    let weak = weak.clone();
                    sheet
                        .size(px(width))
                        .resizable(false)
                        .title(if sidebar {
                            t!("ak.sessions")
                        } else {
                            t!("ak.context_panel")
                        })
                        .child(overlay.clone())
                        .on_close(move |_, _, cx| {
                            let _ = weak.update(cx, |v, cx| {
                                v.sheet_kind = None;
                                if sidebar {
                                    v.sidebar_sheet = false;
                                } else {
                                    v.shell.right_open = false;
                                    v.persist_shell(cx);
                                }
                                cx.notify();
                            });
                        })
                },
            );
        }
    }
}
pub(super) struct ShellOverlay {
    owner: WeakEntity<WorkbenchView>,
    sidebar: bool,
    _observation: Subscription,
}
impl Render for ShellOverlay {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.owner
            .update(cx, |v, cx| {
                if self.sidebar {
                    v.sidebar(cx)
                } else {
                    v.right_panel(cx)
                }
            })
            .unwrap_or_else(|_| div().into_any_element())
    }
}
