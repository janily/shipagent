use super::*;
pub(super) struct Entry {
    pub id: &'static str,
    pub key: String,
    pub context: &'static str,
    pub action: Box<dyn Action>,
}
pub(super) fn defaults() -> Vec<Entry> {
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    macro_rules! row {
        ($id:literal,$key:literal,$scope:literal,$action:expr) => {
            Entry {
                id: $id,
                key: $key.replace("mod", modifier),
                context: $scope,
                action: Box::new($action),
            }
        };
    }
    vec![
        row!("app::NewChat", "mod-n", "Workbench", NewChat),
        row!("app::Settings", "mod-,", "", OpenSettings),
        row!("app::ToggleTheme", "mod-shift-a", "", ToggleTheme),
        row!("app::Search", "mod-f", "Workbench", Search),
        row!("app::KeyboardShortcuts", "mod-/", "", KeyboardShortcuts),
        row!("app::Quit", "mod-q", "", Quit),
        row!("nav::FocusSidebar", "mod-1", "Workbench", FocusSidebar),
        row!("nav::FocusChat", "mod-3", "Workbench", FocusChat),
        row!("nav::GoBack", "mod-[", "Workbench", GoBack),
        row!("nav::GoForward", "mod-]", "Workbench", GoForward),
        row!("view::ToggleSidebar", "mod-b", "Workbench", ToggleSidebar),
        row!(
            "view::ToggleRightPanel",
            "mod-shift-b",
            "Workbench",
            ToggleRightPanel
        ),
        row!(
            "view::ToggleFocusMode",
            "mod-.",
            "Workbench",
            ToggleFocusMode
        ),
        row!(
            "chat::StopProcessing",
            "escape",
            "Workbench",
            StopProcessing
        ),
        row!(
            "chat::CyclePermissionMode",
            "shift-tab",
            "Composer > Input",
            CyclePermissionMode
        ),
        row!(
            "permission::Allow",
            "enter",
            "PermissionPrompt",
            agentkit_ui::permissions::ApprovePermission
        ),
        row!(
            "permission::AllowAlways",
            "mod-enter",
            "PermissionPrompt",
            agentkit_ui::permissions::AlwaysApprovePermission
        ),
        row!(
            "permission::Deny",
            "escape",
            "PermissionPrompt",
            agentkit_ui::permissions::RejectPermission
        ),
        row!(
            "sidebar::Rename",
            "f2",
            "SessionSidebar",
            agentkit_ui::sidebar::RenameAction
        ),
        row!(
            "sidebar::Delete",
            "delete",
            "SessionSidebar",
            agentkit_ui::sidebar::DeleteAction
        ),
        row!("app::CloseWindow", "mod-w", "Workbench", CloseWindow),
        row!("view::ZoomIn", "mod-=", "Workbench", ZoomIn),
        row!("view::ZoomOut", "mod--", "Workbench", ZoomOut),
        row!("view::ActualSize", "mod-0", "Workbench", ActualSize),
    ]
}
/// Localized action label shared by menus and the shortcut list.
pub fn label(id: &str) -> String {
    use agentkit_ui::t;
    match id {
        "app::NewChat" => t!("ak.menu.new_chat"),
        "app::Settings" => t!("ak.settings"),
        "app::ToggleTheme" => t!("ak.menu.theme"),
        "app::Search" => t!("ak.menu.find"),
        "app::KeyboardShortcuts" => t!("ak.menu.shortcuts"),
        "app::Quit" => t!("ak.menu.quit"),
        "nav::FocusSidebar" => t!("ak.menu.focus_sidebar"),
        "nav::FocusChat" => t!("ak.menu.focus_chat"),
        "nav::GoBack" => t!("ak.menu.back"),
        "nav::GoForward" => t!("ak.menu.forward"),
        "view::ToggleSidebar" => t!("ak.toggle_sidebar"),
        "view::ToggleRightPanel" => t!("ak.context_panel"),
        "view::ToggleFocusMode" => t!("ak.focus_mode"),
        "chat::StopProcessing" => t!("ak.menu.stop"),
        "chat::CyclePermissionMode" => t!("ak.menu.cycle_mode"),
        "permission::Allow" => t!("ak.menu.allow"),
        "permission::AllowAlways" => t!("ak.menu.allow_always"),
        "permission::Deny" => t!("ak.menu.deny"),
        "sidebar::Rename" => t!("ak.nav.rename"),
        "sidebar::Delete" => t!("ak.nav.delete"),
        "app::CloseWindow" => t!("ak.menu.close"),
        "view::ZoomIn" => t!("ak.menu.zoom_in"),
        "view::ZoomOut" => t!("ak.menu.zoom_out"),
        "view::ActualSize" => t!("ak.menu.actual_size"),
        _ => id.to_string(),
    }
}
