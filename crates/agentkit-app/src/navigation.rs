use super::*;
use agentkit_ui::sidebar::{NavigationDialog, NavigationForm, SidebarIntent};
#[path = "navigation_mutations.rs"]
mod mutations;
#[path = "navigation_plans.rs"]
mod plans;
#[path = "navigation_search.rs"]
mod search;
#[path = "navigation_workspace.rs"]
mod workspace;
#[derive(Default)]
pub(super) struct State {
    search: Option<Task<()>>,
    commands: std::collections::HashMap<String, (u64, u64)>,
    jump: Option<(String, String)>,
    projection: plans::Projection,
}
impl WorkbenchController {
    pub(super) fn handle_navigation(
        &mut self,
        action: SidebarIntent,
        api: WorkbenchApi,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let SidebarIntent::Search { request, query } = action {
            self.search_navigation(request, query, api, window, cx);
            return;
        }
        if self.loading || self.navigation_pending {
            return;
        }
        match action {
            SidebarIntent::Open(id) => {
                if self.list.read(cx).model.sessions.iter().any(|s| s.id == id) {
                    self.handle(WorkbenchIntent::OpenSession(id), window, cx);
                }
            }
            SidebarIntent::OpenMatch { session_id, query } => {
                if self.list.read(cx).model.sessions.iter().any(|s| {
                    s.id == session_id
                        && s.hidden != Some(true)
                        && self
                            .workspace
                            .as_ref()
                            .is_some_and(|w| w.id == s.workspace_id)
                }) {
                    self.navigation_state.jump = Some((session_id.clone(), query));
                    self.handle(WorkbenchIntent::OpenSession(session_id), window, cx);
                }
            }
            SidebarIntent::Command {
                session_id,
                mutation,
            } => self.mutate_session(session_id, mutation, api, window, cx),
            SidebarIntent::Search { .. } => {}
            SidebarIntent::RetryPlans => self.retry_plan_views(window, cx),
            SidebarIntent::New => self.handle(WorkbenchIntent::NewSession, window, cx),
            SidebarIntent::SwitchWorkspace(id) => {
                let workspace = self
                    .workspace_store
                    .read(cx)
                    .catalog
                    .iter()
                    .find(|w| w.id == id)
                    .cloned();
                if let Some(w) = workspace
                    && self.workspace.as_ref().is_some_and(|old| old.id != id)
                {
                    self.switch_navigation(w, api, window, cx);
                }
            }
            SidebarIntent::Rename(id) => {
                let Some(s) = self
                    .list
                    .read(cx)
                    .model
                    .sessions
                    .iter()
                    .find(|s| s.id == id)
                else {
                    return;
                };
                let name = s.name.clone().unwrap_or_default();
                self.navigation_form(Some(id), &name, window, cx);
            }
            SidebarIntent::CreateWorkspace => self.navigation_form(None, "", window, cx),
            SidebarIntent::Delete(id) => {
                if self.list.read(cx).model.sessions.iter().any(|s| s.id == id) {
                    self.confirm_navigation(false, id, window, cx);
                }
            }
            SidebarIntent::RemoveWorkspace(id) => {
                if self.workspace.as_ref().is_some_and(|w| w.id != id)
                    && self
                        .workspace_store
                        .read(cx)
                        .catalog
                        .iter()
                        .any(|w| w.id == id)
                {
                    self.confirm_navigation(true, id, window, cx);
                }
            }
        }
    }
    fn navigation_form(
        &mut self,
        id: Option<String>,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let form = cx.new(|cx| NavigationDialog::new(name, id.is_none(), window, cx));
        let workspace = id.is_none();
        let epoch = self.epoch;
        self.navigation_subscription = Some(cx.subscribe_in(
            &form,
            window,
            move |this, form, value: &NavigationForm, window, cx| {
                if this.epoch != epoch {
                    return;
                }
                this.mutate_form(id.clone(), form.clone(), value.clone(), window, cx);
            },
        ));
        let focus_form = form.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(if workspace {
                    agentkit_ui::t!("ak.nav.create_workspace")
                } else {
                    agentkit_ui::t!("ak.nav.rename")
                })
                .child(form.clone())
        });
        window.defer(cx, move |window, cx| {
            focus_form.update(cx, |f, cx| f.focus(window, cx))
        });
    }
    fn confirm_navigation(
        &mut self,
        workspace: bool,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use gpui_kit::component::{
            button::{Button, ButtonVariants},
            dialog::DialogFooter,
        };
        let weak = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let weak = weak.clone();
            let id = id.clone();
            dialog
                .title(if workspace {
                    agentkit_ui::t!("ak.nav.remove_workspace")
                } else {
                    agentkit_ui::t!("ak.nav.delete")
                })
                .child(if workspace {
                    agentkit_ui::t!("ak.nav.remove_confirm")
                } else {
                    agentkit_ui::t!("ak.nav.delete_confirm")
                })
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("navigation-cancel")
                                .outline()
                                .label(agentkit_ui::t!("common.cancel"))
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("navigation-confirm")
                                .danger()
                                .label(agentkit_ui::t!("ak.nav.confirm"))
                                .on_click(move |_, window, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.remove_navigation(workspace, id.clone(), window, cx)
                                    });
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }
}

#[cfg(test)]
#[path = "navigation_contract_tests.rs"]
mod contract_tests;
