use super::*;
use gpui_kit::component::{button::Button, switch::Switch, *};

/// Acknowledged backend values, separate from local display preferences.
pub struct BackendPreferences {
    /// Saved proxy configuration, kept out of diagnostics.
    pub proxy: ProxyValues,
    /// Persisted runtime power preference.
    pub keep_awake: bool,
    /// Identity to which permission defaults apply.
    pub workspace_id: String,
    /// Default for new sessions, never a change to an active session.
    pub permission_mode: g::PermissionMode,
    /// Read-only, typed backend policy projection.
    pub permission_policy: String,
    /// Only an owned local sidecar may be restarted.
    pub restart_available: bool,
}
impl SettingsView {
    /// Commit a successful server read and initialize the native proxy editor.
    pub fn backend_preferences(
        &mut self,
        prefs: BackendPreferences,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A read-back of another setting must not replace an unsubmitted proxy draft.
        let preserve_draft = self
            .proxy_form
            .as_ref()
            .is_some_and(|f| f.read(cx).dirty(cx));
        if !preserve_draft {
            let form = cx.new(|cx| {
                proxy::ProxyForm::new(prefs.proxy.clone(), prefs.restart_available, window, cx)
            });
            self.proxy_subscription =
                Some(cx.subscribe(&form, |_, _, values: &ProxyValues, cx| {
                    cx.emit(SettingsIntent::Proxy(values.clone()))
                }));
            self.proxy_form = Some(form);
        }
        self.backend = Some(prefs);
        cx.notify();
    }
    /// Release a pending proposal after confirmation cancellation or a failed save.
    pub fn proxy_finished(&mut self, saved: bool, cx: &mut Context<Self>) {
        if let Some(form) = &self.proxy_form {
            form.update(cx, |s, cx| s.finished(saved, cx));
        }
    }
    pub(super) fn permissions(&self, cx: &mut Context<Self>) -> AnyElement {
        let ready = !self.busy
            && self.backend.is_some()
            && self.app.as_ref().is_some_and(|a| a.read(cx).connected);
        let mode = self.backend.as_ref().map(|b| b.permission_mode);
        let id = self
            .backend
            .as_ref()
            .map(|b| b.workspace_id.clone())
            .unwrap_or_default();
        v_flex()
            .gap_4()
            .child(t!("ak.settings.workspace_permissions"))
            .child(
                h_flex().gap_2().flex_wrap().children(
                    [
                        (g::PermissionMode::Safe, t!("ak.safe")),
                        (g::PermissionMode::Ask, t!("ak.mode_ask")),
                        (g::PermissionMode::AllowAll, t!("ak.mode_execute")),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(i, (value, label))| {
                        let id = id.clone();
                        Button::new(("settings-permission-mode", i))
                            .label(label)
                            .selected(mode == Some(value))
                            .disabled(!ready || mode == Some(value))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(SettingsIntent::PermissionMode(id.clone(), value))
                            }))
                    }),
                ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("ak.settings.permission_hint")),
            )
            .child(
                div()
                    .font_semibold()
                    .child(t!("ak.settings.builtin_policy")),
            )
            .child(
                div()
                    .id("settings-permission-policy")
                    .test_support()
                    .role(Role::Group)
                    .aria_label(t!("ak.settings.builtin_policy"))
                    .text_sm()
                    .font_family(cx.theme().mono_font_family.clone())
                    .child(
                        self.backend
                            .as_ref()
                            .map(|b| b.permission_policy.clone())
                            .unwrap_or_else(|| t!("ak.settings.loading")),
                    ),
            )
            .into_any_element()
    }
    pub(super) fn network(&self) -> AnyElement {
        match &self.proxy_form {
            Some(form) if self.backend.is_some() => div().child(form.clone()).into_any_element(),
            _ => div().child(t!("ak.settings.loading")).into_any_element(),
        }
    }
    pub(super) fn notifications(&self, cx: &mut Context<Self>) -> AnyElement {
        let ready = !self.busy
            && self.backend.is_some()
            && self.app.as_ref().is_some_and(|a| a.read(cx).connected);
        let awake = self.backend.as_ref().is_some_and(|b| b.keep_awake);
        let complete = !self.prefs.completion_notifications;
        let approval = !self.prefs.approval_notifications;
        v_flex()
            .gap_4()
            .child(
                Switch::new("settings-notify-complete")
                    .label(t!("ak.settings.notify_complete"))
                    .checked(!complete)
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(SettingsIntent::Local(LocalChange::CompletionNotifications(
                            complete,
                        )))
                    })),
            )
            .child(
                Switch::new("settings-notify-approval")
                    .label(t!("ak.settings.notify_approval"))
                    .checked(!approval)
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(SettingsIntent::Local(LocalChange::ApprovalNotifications(
                            approval,
                        )))
                    })),
            )
            .child(
                Switch::new("settings-keep-awake")
                    .label(t!("ak.settings.keep_awake"))
                    .checked(awake)
                    .disabled(!ready)
                    .on_click(
                        cx.listener(move |_, _, _, cx| cx.emit(SettingsIntent::KeepAwake(!awake))),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("ak.settings.power_scope")),
            )
            .into_any_element()
    }
    pub(super) fn advanced(&self, cx: &mut Context<Self>) -> AnyElement {
        let restart = !self.busy
            && self.backend.as_ref().is_some_and(|b| b.restart_available)
            && self.app.as_ref().is_some_and(|a| a.read(cx).connected);
        v_flex()
            .gap_4()
            .child(
                Button::new("settings-open-logs")
                    .label(t!("ak.recovery.logs"))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsIntent::OpenLogs))),
            )
            .child(
                Button::new("settings-export-diagnostics")
                    .label(t!("ak.recovery.export"))
                    .on_click(
                        cx.listener(|_, _, _, cx| cx.emit(SettingsIntent::ExportDiagnostics)),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("ak.settings.diagnostic_privacy")),
            )
            .child(
                Button::new("settings-restart-backend")
                    .label(t!("ak.recovery.restart"))
                    .disabled(!restart)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsIntent::RestartBackend))),
            )
            .into_any_element()
    }
}
