use crate::actions::*;
use gpui_kit::*;
pub(crate) fn bind(cx: &mut App) {
    let keys = if cfg!(target_os = "macos") {
        ["cmd-q", "cmd-b", "cmd-shift-b", "cmd-.", "cmd-n"]
    } else {
        ["ctrl-q", "ctrl-b", "ctrl-shift-b", "ctrl-.", "ctrl-n"]
    };
    cx.bind_keys([
        KeyBinding::new(keys[0], Quit, None),
        KeyBinding::new(keys[1], ToggleSidebar, Some("Workbench")),
        KeyBinding::new(keys[2], ToggleRightPanel, Some("Workbench")),
        KeyBinding::new(keys[3], ToggleFocusMode, Some("Workbench")),
        KeyBinding::new(keys[4], NewChat, Some("Workbench")),
    ]);
}
