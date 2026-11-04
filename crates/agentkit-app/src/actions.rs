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
        /// Open native model, appearance and language settings.
        OpenSettings,
        /// Create a conversation through the existing controller.
        NewChat,
        /// Cancel the active session turn when no dialog/sheet consumes Escape.
        StopProcessing
    ]
);
