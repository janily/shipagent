use super::*;
use gpui_kit::component::{button::Button, menu::DropdownMenu, *};
use serde::Deserialize;
#[derive(Clone, PartialEq, Deserialize, Action)]
#[action(namespace=navigation,no_json)]
struct Switch {
    id: String,
}
#[derive(Clone, PartialEq, Deserialize, Action)]
#[action(namespace=navigation,no_json)]
struct Remove {
    id: String,
}
gpui_kit::actions!(navigation, [CreateWorkspace]);
pub(super) struct WorkspaceSwitcher {
    store: Option<Entity<WorkspaceStore>>,
    pub ready: bool,
    subscription: Option<Subscription>,
    focus: FocusHandle,
}
impl EventEmitter<SidebarIntent> for WorkspaceSwitcher {}
impl WorkspaceSwitcher {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            store: None,
            ready: false,
            subscription: None,
            focus: cx.focus_handle(),
        }
    }
    pub fn bind(&mut self, store: Entity<WorkspaceStore>, cx: &mut Context<Self>) {
        self.subscription = Some(cx.observe(&store, |_, _, cx| cx.notify()));
        self.store = Some(store);
    }
}
impl Render for WorkspaceSwitcher {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (id, name, catalog) = self
            .store
            .as_ref()
            .map(|s| {
                let s = s.read(cx);
                (s.id.clone(), s.name.clone(), s.catalog.clone())
            })
            .unwrap_or_default();
        let ready = self.ready;
        let focus = self.focus.clone();
        div()
            .id("workspace-region")
            .track_focus(&self.focus)
            .w_full()
            .on_action(cx.listener(|this, a: &Switch, _, cx| {
                if this.ready {
                    cx.emit(SidebarIntent::SwitchWorkspace(a.id.clone()));
                }
            }))
            .on_action(cx.listener(|this, a: &Remove, _, cx| {
                if this.ready && this.store.as_ref().is_some_and(|s| s.read(cx).id != a.id) {
                    cx.emit(SidebarIntent::RemoveWorkspace(a.id.clone()));
                }
            }))
            .on_action(cx.listener(|this, _: &CreateWorkspace, _, cx| {
                if this.ready {
                    cx.emit(SidebarIntent::CreateWorkspace);
                }
            }))
            .child(
                Button::new("workspace-switcher")
                    .outline()
                    .w_full()
                    .justify_start()
                    .dropdown_caret(true)
                    .label(if name.is_empty() {
                        t!("ak.nav.workspace")
                    } else {
                        name
                    })
                    .accessibility_label(t!("ak.nav.workspace"))
                    .disabled(!ready)
                    .dropdown_menu_with_anchor(Anchor::TopLeft, move |mut menu, _, _| {
                        menu = menu.action_context(focus.clone());
                        for w in &catalog {
                            menu = menu.menu_with_check(
                                w.name.clone(),
                                w.id == id,
                                Box::new(Switch { id: w.id.clone() }),
                            );
                        }
                        menu = menu
                            .separator()
                            .menu(t!("ak.nav.create_workspace"), Box::new(CreateWorkspace));
                        for w in &catalog {
                            menu = menu.menu_with_disabled(
                                t!("ak.nav.remove_named", name = w.name.as_str()),
                                Box::new(Remove { id: w.id.clone() }),
                                w.id == id,
                            );
                        }
                        menu.label(t!("ak.nav.switch_before_remove"))
                    }),
            )
    }
}
