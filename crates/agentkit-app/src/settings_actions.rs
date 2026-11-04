use super::*;
use agentkit_ui::settings::{
    ConnectionEditor, ConnectionKind, ConnectionValues, DisplayPreferences, EditorIntent,
    LocalChange, SettingsIntent,
};
#[path = "settings_connection.rs"]
mod connection;
#[path = "settings_delete.rs"]
mod delete;
#[path = "settings_display.rs"]
mod display;
#[path = "settings_editor.rs"]
mod editor;
#[path = "settings_submission.rs"]
mod submission;
impl WorkbenchController {
    fn settings_status(
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
    fn settings_api(&self) -> Option<WorkbenchApi> {
        let (api, snapshot) = self.api()?.bind_connection().ok()?;
        transport_is_current(&snapshot, self.transport_epoch).then_some(api)
    }
    pub(super) fn handle_settings(
        &mut self,
        action: SettingsIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let SettingsIntent::Back = action {
            window.close_sheet(cx);
            self.view.update(cx, |v, cx| v.close_settings(window, cx));
            return;
        }
        if let SettingsIntent::Local(change) = action {
            self.local_setting(change, window, cx);
            return;
        }
        if self.settings_pending {
            return;
        }
        let Some(api) = self.settings_api() else {
            self.settings_status(
                false,
                Some(agentkit_ui::t!("ak.connection_unavailable")),
                None,
                cx,
            );
            return;
        };
        match action {
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
                            ))
                        }
                        .await;
                        let _ = weak.update_in(cx, |this, _, cx| {
                            if this.epoch != epoch {
                                return;
                            }
                            this.settings_pending = false;
                            match result {
                                Ok((catalog, (thinking, compact), providers)) => {
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
            SettingsIntent::Back | SettingsIntent::Local(_) => {}
        }
    }
}
