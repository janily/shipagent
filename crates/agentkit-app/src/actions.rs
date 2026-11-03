//! Application shell actions; native menus/keymap customization follow later tasks.
gpui_kit::actions!(
    workbench,
    [
        /// Quit after orderly persistence and backend shutdown.
        Quit,
        /// Toggle sidebar or its narrow viewport sheet.
        ToggleSidebar,
        /// Toggle the right context region.
        ToggleRightPanel,
        /// Hide or restore side regions.
        ToggleFocusMode,
        /// Create a conversation through the existing controller.
        NewChat
    ]
);
