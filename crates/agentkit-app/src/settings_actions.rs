use super::*;
use agentkit_ui::settings::{
    ConnectionEditor, ConnectionKind, ConnectionValues, DisplayPreferences, EditorIntent,
    LocalChange, SettingsIntent,
};
#[path = "settings_connection.rs"]
pub(super) mod connection;
#[path = "settings_delete.rs"]
mod delete;
#[path = "settings_display.rs"]
mod display;
#[path = "settings_editor.rs"]
mod editor;
#[path = "settings_submission.rs"]
mod submission;
#[path = "settings_system.rs"]
mod system;
impl WorkbenchController {
    pub(in crate::controller) fn settings_status(
        &self,
        busy: bool,
        error: Option<String>,
        notice: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.view
            .read(cx)
            .settings()
            .update(cx, |s, cx| s.status(busy, error, notice, cx));
    }
    pub(in crate::controller) fn settings_api(&self, cx: &App) -> Option<WorkbenchApi> {
        if !self.view.read(cx).connection_ready() {
            return None;
        }
        let (api, snapshot) = self.api()?.bind_connection().ok()?;
        transport_is_current(&snapshot, self.transport_epoch).then_some(api)
    }
    pub(super) fn handle_settings(
        &mut self,
        action: SettingsIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(action, SettingsIntent::StartClaude) {
            self.start_claude(window, cx);
            return;
        }
        if matches!(action, SettingsIntent::CancelChatGpt) {
            self.oauth_cancel.take();
            return;
        }
        if matches!(action, SettingsIntent::StartChatGpt) {
            self.start_chatgpt(window, cx);
            return;
        }
        if let SettingsIntent::Back = action {
            self.oauth_cancel.take();
            self.close_claude(window, cx);
            window.close_sheet(cx);
            self.view.update(cx, |v, cx| v.close_settings(window, cx));
            return;
        }
        if let SettingsIntent::Local(change) = action {
            self.local_setting(change, window, cx);
            return;
        }
        if matches!(
            action,
            SettingsIntent::OpenKeymap | SettingsIntent::ReloadKeymap
        ) {
            self.keymap_file(matches!(action, SettingsIntent::ReloadKeymap), cx);
            return;
        }
        match action {
            SettingsIntent::OpenLogs => {
                self.recovery_logs(cx);
                return;
            }
            SettingsIntent::ExportDiagnostics => {
                self.export_recovery(window, cx);
                return;
            }
            SettingsIntent::RestartBackend => {
                if !self.settings_pending {
                    self.restart_backend(true, window, cx);
                }
                return;
            }
            _ => {}
        }
        if self.settings_pending {
            if matches!(action, SettingsIntent::Proxy(_)) {
                self.view
                    .read(cx)
                    .settings()
                    .update(cx, |s, cx| s.proxy_finished(false, cx));
            }
            return;
        }
        let Some(api) = self.settings_api(cx) else {
            self.view
                .read(cx)
                .settings()
                .update(cx, |s, cx| s.proxy_finished(false, cx));
            self.settings_status(
                false,
                Some(agentkit_ui::t!("ak.connection_unavailable")),
                None,
                cx,
            );
            return;
        };
        match action {
            SettingsIntent::Proxy(values) => self.confirm_proxy(values, window, cx),
            SettingsIntent::KeepAwake(_) | SettingsIntent::PermissionMode(_, _) => {
                self.system_setting(api, action, false, window, cx)
            }
            SettingsIntent::Edit(id) => self.open_connection_editor(id, window, cx),
            SettingsIntent::Delete(id) => self.confirm_connection_delete(id, window, cx),
            SettingsIntent::Reload
            | SettingsIntent::Test(_)
            | SettingsIntent::Default(_)
            | SettingsIntent::Refresh(_)
            | SettingsIntent::Thinking(_)
            | SettingsIntent::Compact(_) => {
                let auth_only = if let SettingsIntent::Test(id) = &action {
                    self.app_state.read(cx).connections.iter().any(|c|&c.slug==id&&c.auth_type==agentkit_protocol::generated::LlmConnectionWithStatusAuthType::Oauth)
                } else {
                    false
                };
                self.settings_pending = true;
                self.settings_status(true, None, None, cx);
                let epoch = self.epoch;
                let transport_epoch = self.transport_epoch;
                let workspace = self
                    .workspace
                    .as_ref()
                    .map(|w| w.id.clone())
                    .unwrap_or_default();
                let owned = self.owned_sidecar();
                self.operations
                    .push(cx.spawn_in(window, async move |weak, cx| {
                        let result: Result<_> = async {
                            match &action {
                                SettingsIntent::Test(id) => api.test_connection(id).await?,
                                SettingsIntent::Default(id) => api.default_connection(id).await?,
                                SettingsIntent::Refresh(id) => {
                                    api.refresh_connection_models(id).await?
                                }
                                SettingsIntent::Thinking(level) => {
                                    api.default_thinking(*level).await?
                                }
                                SettingsIntent::Compact(tokens) => {
                                    api.auto_compact(*tokens).await?
                                }
                                _ => {}
                            }
                            Ok((
                                api.connections_generated().await?,
                                api.model_defaults().await?,
                                api.connection_providers().await?,
                                if matches!(action, SettingsIntent::Reload) {
                                    Some(system::load_backend(&api, &workspace, owned).await?)
                                } else {
                                    None
                                },
                            ))
                        }
                        .await;
                        let _ = weak.update_in(cx, |this, window, cx| {
                            if !this.settings_operation_current(epoch, transport_epoch, cx) {
                                return;
                            }
                            this.settings_pending = false;
                            match result {
                                Ok((catalog, (thinking, compact), providers, backend)) => {
                                    if let Some(backend) = backend {
                                        this.view.read(cx).settings().update(cx, |s, cx| {
                                            s.backend_preferences(backend, window, cx)
                                        });
                                    }
                                    this.settings_providers =
                                        providers.into_iter().map(|p| (p.key, p.label)).collect();
                                    this.app_state.update(cx, |a, cx| {
                                        a.connections = catalog;
                                        a.default_thinking = Some(thinking);
                                        cx.notify();
                                    });
                                    this.view
                                        .read(cx)
                                        .settings()
                                        .update(cx, |s, cx| s.defaults(thinking, compact, cx));
                                    let notice = if matches!(action, SettingsIntent::Reload) {
                                        None
                                    } else {
                                        Some(if auth_only {
                                            agentkit_ui::t!("ak.settings.auth_config_ok")
                                        } else {
                                            agentkit_ui::t!("ak.settings.operation_ok")
                                        })
                                    };
                                    this.settings_status(false, None, notice, cx);
                                }
                                Err(e) => {
                                    this.settings_status(false, Some(e.to_string()), None, cx)
                                }
                            }
                        });
                    }));
            }
            SettingsIntent::StartChatGpt
            | SettingsIntent::StartClaude
            | SettingsIntent::CancelChatGpt => {}
            SettingsIntent::Back
            | SettingsIntent::Local(_)
            | SettingsIntent::OpenKeymap
            | SettingsIntent::ReloadKeymap => {}
            SettingsIntent::OpenLogs
            | SettingsIntent::ExportDiagnostics
            | SettingsIntent::RestartBackend => {}
        }
    }
}
