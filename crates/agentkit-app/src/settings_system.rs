use super::*;
use agentkit_ui::settings::{BackendPreferences, ProxyValues};
use secrecy::ExposeSecret;

pub(super) async fn load_backend(
    api: &WorkbenchApi,
    workspace: &str,
    owned: bool,
) -> Result<BackendPreferences> {
    let proxy = api.network_proxy().await?;
    let keep_awake = api.keep_awake().await?;
    let defaults = api.builtin_permissions().await?;
    let settings = api.workspace_settings(workspace).await?;
    Ok(BackendPreferences {
        proxy: ProxyValues {
            enabled: proxy.enabled,
            http_proxy: proxy.http_proxy.unwrap_or_else(|| String::new().into()),
            https_proxy: proxy.https_proxy.unwrap_or_else(|| String::new().into()),
            no_proxy: proxy.no_proxy.unwrap_or_default(),
        },
        keep_awake,
        workspace_id: workspace.into(),
        permission_mode: settings
            .and_then(|s| s.permission_mode)
            .unwrap_or(agentkit_protocol::generated::PermissionMode::Safe),
        permission_policy: defaults
            .config
            .map(|c| serde_json::to_string_pretty(&c))
            .transpose()?
            .unwrap_or_else(|| agentkit_ui::t!("ak.settings.policy_unavailable")),
        restart_available: owned,
    })
}
fn valid_proxy(values: &ProxyValues) -> bool {
    [&values.http_proxy, &values.https_proxy].iter().all(|s| {
        let s = s.expose_secret();
        s.is_empty()
            || url::Url::parse(s)
                .is_ok_and(|u| matches!(u.scheme(), "http" | "https") && u.host_str().is_some())
    })
}
impl WorkbenchController {
    pub(super) fn owned_sidecar(&self) -> bool {
        self.runtime
            .borrow()
            .as_ref()
            .is_some_and(|r| r.sidecar.is_some())
    }
    pub(super) fn settings_operation_current(&self, epoch: u64, transport: u64, cx: &App) -> bool {
        self.epoch == epoch && self.transport_epoch == transport && self.settings_api(cx).is_some()
    }
    fn proxy_finished(&self, saved: bool, cx: &mut Context<Self>) {
        self.view
            .read(cx)
            .settings()
            .update(cx, |s, cx| s.proxy_finished(saved, cx));
    }
    pub(super) fn confirm_proxy(
        &mut self,
        values: ProxyValues,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.owned_sidecar() || !valid_proxy(&values) || window.has_active_dialog(cx) {
            self.proxy_finished(false, cx);
            self.settings_status(
                false,
                Some(agentkit_ui::t!("ak.settings.proxy_invalid")),
                None,
                cx,
            );
            return;
        }
        let epoch = self.epoch;
        let transport = self.transport_epoch;
        self.settings_pending = true;
        self.settings_status(true, None, None, cx);
        let weak = cx.weak_entity();
        let accepted = Rc::new(Cell::new(false));
        window.open_alert_dialog(cx, move |alert, _, _| {
            let ok = weak.clone();
            let close = weak.clone();
            let values = values.clone();
            let accepted_ok = accepted.clone();
            let accepted_close = accepted.clone();
            alert
                .confirm()
                .title(agentkit_ui::t!("ak.settings.proxy_restart"))
                .description(agentkit_ui::t!("ak.settings.proxy_restart_confirm"))
                .ok_text(agentkit_ui::t!("ak.settings.apply_restart"))
                .on_ok(move |_, window, cx| {
                    let _ = ok.update(cx, |this, cx| {
                        if this.settings_operation_current(epoch, transport, cx)
                            && let Some(api) = this.settings_api(cx)
                        {
                            accepted_ok.set(true);
                            this.save_proxy(api, values.clone(), window, cx);
                        }
                    });
                    true
                })
                .on_close(move |_, _, cx| {
                    if !accepted_close.get() {
                        let _ = close.update(cx, |this, cx| {
                            if this.epoch == epoch && this.transport_epoch == transport {
                                this.settings_pending = false;
                                this.proxy_finished(false, cx);
                                this.settings_status(false, None, None, cx);
                            }
                        });
                    }
                })
        });
    }
    fn save_proxy(
        &mut self,
        api: WorkbenchApi,
        values: ProxyValues,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let epoch = self.epoch;
        let transport = self.transport_epoch;
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let optional =
                    |v: secrecy::SecretString| (!v.expose_secret().is_empty()).then_some(v);
                let proxy = agentkit_client::api::secrets::NetworkProxy {
                    enabled: values.enabled,
                    http_proxy: optional(values.http_proxy),
                    https_proxy: optional(values.https_proxy),
                    no_proxy: (!values.no_proxy.is_empty()).then_some(values.no_proxy),
                };
                let result = api.save_network_proxy(&proxy).await;
                let _ = weak.update_in(cx, |this, window, cx| {
                    if !this.settings_operation_current(epoch, transport, cx) {
                        return;
                    }
                    this.settings_pending = false;
                    match result {
                        Ok(ack) if ack.restart_required => {
                            this.proxy_finished(true, cx);
                            this.settings_status(
                                false,
                                None,
                                Some(agentkit_ui::t!("ak.settings.proxy_restarting")),
                                cx,
                            );
                            this.restart_backend(false, window, cx);
                        }
                        _ => {
                            this.proxy_finished(false, cx);
                            this.settings_status(
                                false,
                                Some(agentkit_ui::t!("ak.settings.proxy_failed")),
                                None,
                                cx,
                            );
                        }
                    }
                });
            }));
    }
    pub(super) fn system_setting(
        &mut self,
        api: WorkbenchApi,
        action: SettingsIntent,
        confirmed: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use agentkit_protocol::generated::PermissionMode;
        let Some(workspace) = self.workspace.as_ref().map(|w| w.id.clone()) else {
            return;
        };
        if matches!(&action,SettingsIntent::PermissionMode(id,_) if id != &workspace) {
            return;
        }
        let epoch = self.epoch;
        let transport = self.transport_epoch;
        if matches!(
            action,
            SettingsIntent::PermissionMode(_, PermissionMode::AllowAll)
        ) && !confirmed
        {
            if window.has_active_dialog(cx) {
                return;
            }
            self.settings_pending = true;
            self.settings_status(true, None, None, cx);
            let weak = cx.weak_entity();
            let accepted = Rc::new(Cell::new(false));
            window.open_alert_dialog(cx, move |alert, _, _| {
                let ok = weak.clone();
                let close = weak.clone();
                let action = action.clone();
                let accepted_ok = accepted.clone();
                let accepted_close = accepted.clone();
                alert
                    .confirm()
                    .title(agentkit_ui::t!("ak.settings.workspace_permissions"))
                    .description(agentkit_ui::t!("ak.settings.execute_confirm"))
                    .on_ok(move |_, window, cx| {
                        let _ = ok.update(cx, |this, cx| {
                            if this.settings_operation_current(epoch, transport, cx)
                                && let Some(api) = this.settings_api(cx)
                            {
                                accepted_ok.set(true);
                                this.system_setting(api, action.clone(), true, window, cx);
                            }
                        });
                        true
                    })
                    .on_close(move |_, _, cx| {
                        if !accepted_close.get() {
                            let _ = close.update(cx, |this, cx| {
                                if this.epoch == epoch && this.transport_epoch == transport {
                                    this.settings_pending = false;
                                    this.settings_status(false, None, None, cx);
                                }
                            });
                        }
                    })
            });
            return;
        }
        self.settings_pending = true;
        self.settings_status(true, None, None, cx);
        let owned = self.owned_sidecar();
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let result: Result<_> = async {
                    match action {
                        SettingsIntent::KeepAwake(enabled) => api.set_keep_awake(enabled).await?,
                        SettingsIntent::PermissionMode(id, mode) => {
                            api.workspace_permission_mode(&id, mode).await?
                        }
                        _ => return Err(anyhow::anyhow!("Invalid settings action")),
                    }
                    let prefs = load_backend(&api, &workspace, owned).await?;
                    Ok((prefs, api.workspace_settings(&workspace).await?))
                }
                .await;
                let _ = weak.update_in(cx, |this, window, cx| {
                    if !this.settings_operation_current(epoch, transport, cx) {
                        return;
                    }
                    this.settings_pending = false;
                    match result {
                        Ok((prefs, settings)) => {
                            this.workspace_store.update(cx, |s, cx| {
                                s.settings = settings;
                                cx.notify();
                            });
                            this.view
                                .read(cx)
                                .settings()
                                .update(cx, |s, cx| s.backend_preferences(prefs, window, cx));
                            this.settings_status(
                                false,
                                None,
                                Some(agentkit_ui::t!("ak.settings.operation_ok")),
                                cx,
                            );
                        }
                        Err(_) => this.settings_status(
                            false,
                            Some(agentkit_ui::t!("ak.settings.update_failed")),
                            None,
                            cx,
                        ),
                    }
                });
            }));
    }
}

#[cfg(test)]
#[path = "settings_system_tests.rs"]
mod tests;
