//! Shared actions used by native menus, key bindings and the application controller.
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
        StopProcessing,
        /// Toggle the persisted light/dark appearance.
        ToggleTheme,
        /// Cycle permission mode through the existing composer policy.
        CyclePermissionMode,
        /// Focus the sidebar search input.
        Search,
        /// Focus the sidebar list.
        FocusSidebar,
        /// Focus the conversation composer.
        FocusChat,
        /// Navigate to the previous successfully opened session.
        GoBack,
        /// Navigate to the next successfully opened session.
        GoForward,
        /// Show effective shortcuts and keymap controls.
        KeyboardShortcuts,
        /// Open the user-owned keymap with the default editor.
        OpenKeymap,
        /// Reload the keymap without restarting.
        ReloadKeymap,
        /// Create a workspace using the existing controller form.
        NewWorkspace,
        /// Rename the active conversation.
        RenameSession,
        /// Confirm deletion of the active conversation.
        DeleteSession,
        /// Close the window through the orderly quit path.
        CloseWindow,
        /// Exit only after explicit confirmation when sessions are running.
        ConfirmedQuit,
        /// Quit after explicitly discarding unsaved draft changes.
        DiscardDraftsAndQuit,
        /// Increase body text within the supported range.
        ZoomIn,
        /// Decrease body text within the supported range.
        ZoomOut,
        /// Reset body text to fourteen logical pixels.
        ActualSize,
        /// Show versions and license notices.
        About,
        /// Open the platform hide application command.
        Hide,
        /// Open the owned log directory when available.
        OpenLogs,
        /// A disabled entry for a separately scheduled feature.
        DeferredFeature
    ]
);
