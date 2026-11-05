use super::*;
use crate::keymap;
use agentkit_ui::settings::LocalChange;
impl WorkbenchController {
    pub(crate) fn global_command(&mut self, kind: u8, window: &mut Window, cx: &mut Context<Self>) {
        if kind == 2 {
            self.toggle_theme(window, cx);
            return;
        }
        if window.has_active_dialog(cx) {
            return;
        }
        if kind == 1 {
            self.shortcut_page(true, window, cx);
        } else {
            self.shortcut_page(false, window, cx);
            self.view
                .update(cx, |v, cx| v.show_configuration(window, cx));
        }
    }
    pub(crate) fn configure_shortcuts(&mut self, escape: bool, cx: &mut Context<Self>) {
        self.view.update(cx, |v, cx| v.set_escape_stops(escape, cx));
    }
    pub(super) fn shortcut_page(
        &mut self,
        select: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let rows = keymap::rows(cx);
        let invalid = cx.try_global::<keymap::State>().is_some_and(|s| s.invalid);
        self.view.update(cx, |v, cx| {
            if select {
                v.show_configuration(window, cx);
            }
            v.settings()
                .update(cx, |s, cx| s.shortcuts(rows, invalid, select, cx));
        });
    }
    pub(super) fn keymap_file(&mut self, reload: bool, cx: &mut Context<Self>) {
        let Some(path) = cx.try_global::<keymap::State>().map(|s| s.path.clone()) else {
            return;
        };
        let generation = {
            let state = cx.global_mut::<keymap::State>();
            state.generation = state.generation.wrapping_add(1);
            state.generation
        };
        self.operations.push(cx.spawn(async move |weak, cx| {
            if reload {
                let (path, result) = cx
                    .background_executor()
                    .spawn(async move {
                        let result = keymap::read(&path);
                        (path, result)
                    })
                    .await;
                let _ = weak.update_in(cx, |this, window, cx| {
                    if cx.global::<keymap::State>().generation != generation {
                        return;
                    }
                    keymap::install(path, result, cx);
                    this.configure_shortcuts(keymap::escape_stops(cx), cx);
                    this.shortcut_page(false, window, cx);
                    crate::menus::refresh(this.menu_status(cx), cx);
                });
            } else {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        keymap::ensure_file(&path).map_err(|_| ())?;
                        agentkit_capabilities::open_local_path(&path).map_err(|_| ())
                    })
                    .await;
                if result.is_err() {
                    let _ = weak.update(cx, |this, cx| {
                        this.view.read(cx).settings().update(cx, |s, cx| {
                            s.status(
                                false,
                                Some(agentkit_ui::t!("ak.menu.open_failed")),
                                None,
                                cx,
                            )
                        })
                    });
                }
            }
        }));
    }
    pub(super) fn menu_status(&self, cx: &App) -> crate::menus::Status {
        crate::menus::Status {
            ready: self.api().is_some() && !self.loading && !self.navigation_pending,
            session: self.state.is_some(),
            busy: self.state.as_ref().is_some_and(|s| s.read(cx).model.busy()),
            blocked: !self.view.read(cx).conversation_visible(),
            onboarding: self.view.read(cx).onboarding_visible(),
        }
    }
    pub(super) fn menu_focus(
        &mut self,
        sidebar: bool,
        search: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !window.has_active_dialog(cx) {
            self.view
                .update(cx, |v, cx| v.command_focus(sidebar, search, window, cx));
        }
    }
    pub(super) fn history_step(
        &mut self,
        delta: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.menu_status(cx).ready
            || !self.view.read(cx).conversation_visible()
            || window.has_active_dialog(cx)
            || window.has_active_sheet(cx)
        {
            return;
        }
        let list = &self.list.read(cx).model.sessions;
        let id = self
            .history
            .borrow_mut()
            .step(delta, |id| list.iter().any(|s| s.id == id));
        if let Some(id) = id {
            self.handle(WorkbenchIntent::OpenSession(id), window, cx);
        }
    }
    pub(super) fn change_font(&mut self, delta: i8, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.preferences.value.borrow().font_size;
        let size = if delta == 0 {
            14
        } else {
            current.saturating_add_signed(delta).clamp(12, 16)
        };
        self.local_setting(LocalChange::Font(size), window, cx);
    }
    pub(super) fn toggle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_kit::component::ActiveTheme;
        let appearance = if cx.theme().mode.is_dark() {
            Appearance::Light
        } else {
            Appearance::Dark
        };
        self.local_setting(LocalChange::Appearance(appearance), window, cx);
    }
    pub(super) fn session_command(
        &mut self,
        delete: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.has_active_dialog(cx)
            || window.has_active_sheet(cx)
            || !self.view.read(cx).conversation_visible()
        {
            return;
        }
        if let Some(id) = self
            .state
            .as_ref()
            .map(|s| s.read(cx).model.state.session.id.clone())
        {
            self.handle(
                WorkbenchIntent::Sidebar(if delete {
                    agentkit_ui::sidebar::SidebarIntent::Delete(id)
                } else {
                    agentkit_ui::sidebar::SidebarIntent::Rename(id)
                }),
                window,
                cx,
            );
        }
    }
}
