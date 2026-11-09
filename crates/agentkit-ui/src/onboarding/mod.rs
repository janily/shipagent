//! Full-window first-run flow. Credentials remain in the embedded native editor.
use crate::{
    settings::{ConnectionEditor, ConnectionKind},
    t,
};
use gpui_kit::{prelude::*, *};
mod methods;
mod render;
/// Wizard gestures, handled by the app controller.
#[derive(Clone)]
pub enum OnboardingIntent {
    /// Select an available non-OAuth connection method.
    Choose(ConnectionKind),
    /// Return from the credential form, dropping its secrets.
    Back,
    /// Start a ChatGPT Plus browser login.
    ChatGpt,
    /// Cancel the active browser login.
    CancelChatGpt,
    /// Persist the user's request to configure later.
    Defer,
    /// Enter the first conversation after saving and selecting a default.
    Finish,
}
/// Full-window wizard with explicit acknowledged completion.
pub struct OnboardingView {
    step: usize,
    busy: bool,
    oauth_pending: bool,
    editor: Option<Entity<ConnectionEditor>>,
    error: Option<String>,
    focus: FocusHandle,
}
impl EventEmitter<OnboardingIntent> for OnboardingView {}
impl OnboardingView {
    /// Reflect window-owned browser login, with an always reachable cancel action.
    pub fn oauth_pending(&mut self, value: bool, cx: &mut Context<Self>) {
        self.oauth_pending = value;
        self.busy = value;
        cx.notify();
    }

    /// Start at welcome without fetching credentials or configuration.
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            step: 0,
            busy: false,
            oauth_pending: false,
            editor: None,
            error: None,
            focus: cx.focus_handle(),
        }
    }
    /// Install an owned credential form after provider discovery.
    pub fn edit(&mut self, editor: Entity<ConnectionEditor>, cx: &mut Context<Self>) {
        self.editor = Some(editor);
        self.step = 2;
        self.busy = false;
        self.error = None;
        cx.notify();
    }
    /// Return to methods after dropping the form in the controller.
    pub fn methods(&mut self, cx: &mut Context<Self>) {
        self.editor = None;
        self.step = 1;
        self.busy = false;
        self.error = None;
        cx.notify();
    }
    /// Advance only after successful save and default selection.
    pub fn completed(&mut self, cx: &mut Context<Self>) {
        self.editor = None;
        self.step = 3;
        self.busy = false;
        self.error = None;
        cx.notify();
    }
    /// Serialize backend operations and expose failure reasons.
    pub fn status(&mut self, busy: bool, error: Option<String>, cx: &mut Context<Self>) {
        self.busy = busy;
        self.error = error;
        cx.notify();
    }
}
/// Port of resolveSlugForMethod from mkagent v0.2.6
/// apps/electron/src/renderer/hooks/useOnboarding.ts (non-OAuth methods).
pub fn resolve_slug(existing: impl IntoIterator<Item = String>) -> String {
    let existing: std::collections::HashSet<_> = existing.into_iter().collect();
    let base = "pi-api-key";
    if !existing.contains(base) {
        return base.into();
    }
    let mut n = 2;
    while existing.contains(&format!("{base}-{n}")) {
        n += 1;
    }
    format!("{base}-{n}")
}

#[cfg(feature = "e2e")]
impl OnboardingView {
    /// Read the rendered wizard step in an opt-in native E2E build.
    pub fn e2e_step(&self) -> usize {
        self.step
    }
}
