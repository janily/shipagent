# Native menus and shortcuts

Stage 1 commands share the `agentkit_app::actions` handlers. macOS uses the OS menu; Windows/Linux display Kit AppMenuBar from the same tree. Deferred phase 2/3 commands are disabled. Menus rebuild after language, command gates or keymap revision changes.

The client profile contains `keymap.json` beside `prefs.json`. Open **Settings → Keyboard shortcuts → Open keymap.json**, edit, then reload:

```json
{
  "version": 1,
  "bindings": [
    { "action": "app::NewChat", "key": "ctrl-k" },
    { "action": "app::Search", "key": null }
  ]
}
```

Keys use GPUI syntax: `ctrl-k`, `cmd-k`, `alt-shift-k`, or a space-separated chord. Use `cmd` on macOS and `ctrl` on Windows/Linux. Null removes the action's defaults, including the sidebar Delete/Backspace alias. Unassigned actions disappear from the effective list. The app does not permit changing an action's focus scope through this file. Invalid JSON, version, action, key, duplicate action/shortcut or an oversized file restores defaults and preserves the original file. No external command is accepted as an action.

Supported IDs follow the 04 specification: app::NewChat, Settings, ToggleTheme, Search, KeyboardShortcuts, Quit; nav::FocusSidebar, FocusChat, GoBack, GoForward; view::ToggleSidebar, ToggleRightPanel, ToggleFocusMode; chat::StopProcessing, CyclePermissionMode; permission::Allow, AllowAlways, Deny; sidebar::Rename, Delete. Additional window commands are app::CloseWindow and view::ZoomIn, ZoomOut, ActualSize. Each name after a comma keeps the most recent namespace prefix.

History is window-local and resets on workspace changes; successful opens commit the cursor, failed loads leave it unchanged, and new navigation discards the forward branch. Deleted sessions are skipped. Focus commands reveal responsive sidebars. A model-free conversation focuses its rendered center instead of the absent composer. Escape and Shift+Tab retain IME/popup protection and respect keymap removals.

On Linux, with a display/Vulkan/xdotool/Python Pillow available:

```sh
cargo build -p workbench --locked
node tools/menus/smoke.mjs target/debug/workbench target/backend/linux-x64 target/menus.json
```

The output must not exist. This uses an isolated restored/deferred profile, ordinary workbench and pinned backend, with no provider credentials. It checks menus, sidebar focus, new/back/forward, light/dark switching, live keymap reload, invalid-file fallback and owned process cleanup. Screenshots are separate from physical desktop/performance acceptance.
