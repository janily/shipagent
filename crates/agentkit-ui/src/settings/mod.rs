//! Native settings and connection editor; all IO is delegated through semantic intents.
mod connection_rows;
mod editor;
mod editor_fields;
mod editor_render;
mod editor_types;
mod models;
mod preferences;
mod render;
use crate::{composer::SendKey, t};
use agentkit_protocol::generated as g;
use agentkit_store::entities::AppState;
pub use editor::ConnectionEditor;
pub use editor_types::{ConnectionKind, ConnectionValues, EditorIntent};
use gpui_kit::{prelude::*, *};
use serde::{Deserialize, Serialize};
/// Client appearance preference; system follows the desktop dynamically.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    /// Desktop appearance.
    System,
    /// Bundled light palette.
    #[default]
    Light,
    /// Bundled dark palette.
    Dark,
}
/// Non-secret preferences exposed by the native settings UI.
#[derive(Clone, PartialEq)]
pub struct DisplayPreferences {
    /// Appearance mode.
    pub appearance: Appearance,
    /// Owned UI locale.
    pub language: String,
    /// Body font size (12–16 logical pixels).
    pub font_size: u8,
    /// Fade newly streamed Markdown.
    pub stream_fade: bool,
    /// Restore the last session at launch.
    pub restore: bool,
    /// Send gesture.
    pub send_key: SendKey,
}
impl Default for DisplayPreferences {
    fn default() -> Self {
        Self {
            appearance: Appearance::Light,
            language: "zh-CN".into(),
            font_size: 14,
            stream_fade: true,
            restore: true,
            send_key: SendKey::Enter,
        }
    }
}
/// A single local change avoids overwriting concurrent preferences.
#[derive(Clone)]
pub enum LocalChange {
    /// Select mode.
    Appearance(Appearance),
    /// Select locale.
    Language(String),
    /// Select font size.
    Font(u8),
    /// Toggle stream fade.
    Fade(bool),
    /// Toggle launch restoration.
    Restore(bool),
    /// Change send key.
    SendKey(SendKey),
}
/// Settings gestures; no RPC channel or saved credential belongs in the view.
#[derive(Clone)]
pub enum SettingsIntent {
    /// Return to the conversation.
    Back,
    /// Load server-owned settings.
    Reload,
    /// Persist a local setting immediately.
    Local(LocalChange),
    /// Open a new or existing connection editor.
    Edit(Option<String>),
    /// Confirm deletion.
    Delete(String),
    /// Test a saved credential reference.
    Test(String),
    /// Set a default.
    Default(String),
    /// Synchronize models.
    Refresh(String),
    /// Set default reasoning.
    Thinking(g::ThinkingLevel),
    /// Set optional compaction token threshold.
    Compact(Option<u32>),
}
/// Native Settings component with client values and a safe canonical connection catalog.
pub struct SettingsView {
    focus: FocusHandle,
    prefs: DisplayPreferences,
    locale_generation: usize,
    app: Option<Entity<AppState>>,
    busy: bool,
    loaded: bool,
    thinking: g::ThinkingLevel,
    compact: Option<u32>,
    error: Option<String>,
    notice: Option<String>,
    _subscription: Option<Subscription>,
}
impl EventEmitter<SettingsIntent> for SettingsView {}
impl SettingsView {
    /// Construct without fetching any data.
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            prefs: Default::default(),
            locale_generation: 0,
            app: None,
            busy: false,
            loaded: false,
            thinking: g::ThinkingLevel::Medium,
            compact: None,
            error: None,
            notice: None,
            _subscription: None,
        }
    }
    /// Move focus away from hidden conversation inputs when settings opens.
    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
    }
    /// Attach the canonical credential-free connection list.
    pub fn bind(&mut self, app: Entity<AppState>, cx: &mut Context<Self>) {
        self._subscription = Some(cx.observe(&app, |_, _, cx| cx.notify()));
        self.app = Some(app);
        cx.notify();
    }
    /// Refresh local controls after the controller applies preferences.
    pub fn preferences(&mut self, prefs: DisplayPreferences, cx: &mut Context<Self>) {
        if self.prefs.language != prefs.language {
            self.locale_generation = self.locale_generation.wrapping_add(1);
        }
        self.prefs = prefs;
        cx.notify();
    }
    /// Invalidate server controls when a transport generation is replaced.
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.loaded = false;
        self.busy = false;
        self.error = None;
        self.notice = None;
        cx.notify();
    }
    /// Apply server defaults only after a successful read or write.
    pub fn defaults(
        &mut self,
        thinking: g::ThinkingLevel,
        compact: Option<u32>,
        cx: &mut Context<Self>,
    ) {
        self.thinking = thinking;
        self.compact = compact;
        self.loaded = true;
        cx.notify();
    }
    /// Serialize mutations and surface a recoverable result.
    pub fn status(
        &mut self,
        busy: bool,
        error: Option<String>,
        notice: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.busy = busy;
        self.error = error;
        self.notice = notice;
        cx.notify();
    }
}
fn emit(owner: &WeakEntity<SettingsView>, intent: SettingsIntent, cx: &mut App) {
    let _ = owner.update(cx, |_, cx| cx.emit(intent));
}
