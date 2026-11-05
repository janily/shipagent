//! One menu tree for the platform menu and the Kit application bar.
use crate::{actions::*, keymap::label};
use agentkit_ui::t;
use gpui_kit::{
    component::{GlobalState, menu::AppMenuBar},
    *,
};
/// Current window command gates; deferred features remain visibly unavailable.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Status {
    pub ready: bool,
    pub session: bool,
    pub busy: bool,
    pub blocked: bool,
    pub onboarding: bool,
}
struct State {
    title: String,
    bar: Entity<AppMenuBar>,
    status: Status,
    locale: String,
    revision: u64,
}
impl Global for State {}
pub(crate) fn init(title: String, cx: &mut App) -> Entity<AppMenuBar> {
    let bar = AppMenuBar::new(cx);
    cx.set_global(State {
        title,
        bar: bar.clone(),
        status: Status::default(),
        locale: String::new(),
        revision: 0,
    });
    refresh(Status::default(), cx);
    bar
}
pub(crate) fn refresh(status: Status, cx: &mut App) {
    let Some(state) = cx.try_global::<State>() else {
        return;
    };
    let locale = rust_i18n::locale().to_string();
    let revision = cx
        .try_global::<crate::keymap::State>()
        .map_or(0, |s| s.revision);
    if state.status == status && state.locale == locale && state.revision == revision {
        return;
    }
    let (title, bar) = (state.title.clone(), state.bar.clone());
    let menus = build(&title, status);
    let owned = build(&title, status).into_iter().map(Menu::owned).collect();
    cx.set_menus(menus);
    GlobalState::global_mut(cx).set_app_menus(owned);
    bar.update(cx, |b, cx| b.reload(cx));
    let state = cx.global_mut::<State>();
    state.status = status;
    state.locale = locale;
    state.revision = revision;
}
fn item<A: Action>(id: &str, a: A) -> MenuItem {
    MenuItem::action(label(id), a)
}
fn later(name: String) -> MenuItem {
    MenuItem::action(name, DeferredFeature).disabled(true)
}
pub(crate) fn build(title: &str, s: Status) -> Vec<Menu> {
    use gpui_kit::component::input;
    let disabled = s.blocked;
    let no_session = disabled || !s.ready || !s.session;
    let mut file = vec![
        item("app::NewChat", NewChat).disabled(s.onboarding || !s.ready),
        later(t!("ak.menu.new_window")),
        MenuItem::action(t!("ak.menu.new_workspace"), NewWorkspace).disabled(disabled || !s.ready),
        MenuItem::Separator,
        later(t!("ak.menu.import")),
        later(t!("ak.menu.export")),
        MenuItem::Separator,
        item("app::CloseWindow", CloseWindow),
    ];
    if !cfg!(target_os = "macos") {
        file.extend([
            MenuItem::Separator,
            item("app::Settings", OpenSettings).disabled(s.onboarding),
            item("app::Quit", Quit),
        ]);
    }
    let mut help = vec![
        item("app::KeyboardShortcuts", KeyboardShortcuts).disabled(s.onboarding),
        later(t!("ak.menu.logs")),
        later(t!("ak.menu.diagnostics")),
        MenuItem::action(t!("ak.menu.licenses"), About),
    ];
    if !cfg!(target_os = "macos") {
        help.push(MenuItem::action(t!("ak.menu.about"), About));
    }
    let mut menus = vec![
        Menu::new(t!("ak.menu.file")).items(file),
        Menu::new(t!("ak.menu.edit")).items([
            MenuItem::os_action(t!("ak.menu.undo"), input::Undo, OsAction::Undo),
            MenuItem::os_action(t!("ak.menu.redo"), input::Redo, OsAction::Redo),
            MenuItem::Separator,
            MenuItem::os_action(t!("ak.menu.cut"), input::Cut, OsAction::Cut),
            MenuItem::os_action(t!("ak.menu.copy"), input::Copy, OsAction::Copy),
            MenuItem::os_action(t!("ak.menu.paste"), input::Paste, OsAction::Paste),
            MenuItem::os_action(
                t!("ak.menu.select_all"),
                input::SelectAll,
                OsAction::SelectAll,
            ),
            MenuItem::Separator,
            item("app::Search", Search).disabled(s.onboarding),
        ]),
        Menu::new(t!("ak.menu.view")).items([
            item("view::ToggleSidebar", ToggleSidebar).disabled(disabled),
            item("view::ToggleRightPanel", ToggleRightPanel).disabled(disabled),
            item("view::ToggleFocusMode", ToggleFocusMode).disabled(disabled),
            item("app::ToggleTheme", ToggleTheme),
            MenuItem::Separator,
            item("nav::FocusSidebar", FocusSidebar).disabled(s.onboarding),
            item("nav::FocusChat", FocusChat).disabled(s.onboarding),
            item("nav::GoBack", GoBack).disabled(disabled),
            item("nav::GoForward", GoForward).disabled(disabled),
            MenuItem::Separator,
            item("view::ZoomIn", ZoomIn),
            item("view::ZoomOut", ZoomOut),
            item("view::ActualSize", ActualSize),
        ]),
        Menu::new(t!("ak.menu.session")).items([
            item("chat::StopProcessing", StopProcessing).disabled(no_session || !s.busy),
            item("chat::CyclePermissionMode", CyclePermissionMode).disabled(no_session),
            MenuItem::action(t!("ak.nav.rename"), RenameSession).disabled(no_session),
            later(t!("ak.menu.pin")),
            later(t!("ak.menu.archive")),
            later(t!("ak.menu.branch")),
            MenuItem::Separator,
            MenuItem::action(t!("ak.nav.delete"), DeleteSession).disabled(no_session),
        ]),
        Menu::new(t!("ak.menu.help")).items(help),
    ];
    if cfg!(target_os = "macos") {
        menus.insert(
            0,
            Menu::new(title.to_string()).items([
                MenuItem::action(t!("ak.menu.about"), About),
                item("app::Settings", OpenSettings).disabled(s.onboarding),
                later(t!("ak.menu.update")),
                MenuItem::Separator,
                MenuItem::action(t!("ak.menu.hide"), Hide),
                item("app::Quit", Quit),
            ]),
        );
    }
    menus
}

pub(crate) fn bar(cx: &App) -> Option<Entity<AppMenuBar>> {
    if cfg!(target_os = "macos") {
        None
    } else {
        cx.try_global::<State>().map(|s| s.bar.clone())
    }
}

pub(crate) fn keymap_changed(cx: &mut App) {
    if let Some(state) = cx.try_global::<State>() {
        let status = state.status;
        refresh(status, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::core::prelude::v1::test;
    #[gpui_kit::test]
    fn keymap_install_rebuilds_native_menus_even_without_a_business_state_change(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            init("Workbench".into(), cx);
            crate::keymap::install("keymap.json".into(), Ok(None), cx);
            let first = cx.global::<State>().revision;
            assert_eq!(first, cx.global::<crate::keymap::State>().revision);
            crate::keymap::install(
                "keymap.json".into(),
                Ok(Some(
                    r#"{"version":1,"bindings":[{"action":"app::NewChat","key":"ctrl-k"}]}"#.into(),
                )),
                cx,
            );
            assert!(cx.global::<State>().revision > first);
            assert_eq!(
                cx.global::<State>().revision,
                cx.global::<crate::keymap::State>().revision
            );
        });
    }
    #[gpui_kit::test]
    fn unavailable_commands_stay_disabled_and_edit_actions_use_input_dispatch(
        _: &mut TestAppContext,
    ) {
        let idle = build("Workbench", Status::default());
        let items = idle.iter().flat_map(|m| &m.items);
        let mut deferred = 0;
        let mut input_copy = false;
        for item in items {
            if let MenuItem::Action {
                action,
                disabled,
                os_action,
                ..
            } = item
            {
                if action.as_any().is::<DeferredFeature>() {
                    assert!(*disabled);
                    deferred += 1;
                }
                if action.as_any().is::<NewChat>() {
                    assert!(*disabled);
                }
                if action.as_any().is::<gpui_kit::component::input::Copy>() {
                    input_copy = matches!(os_action, Some(OsAction::Copy));
                }
            }
        }
        assert!(deferred >= 6 && input_copy);
    }
}
