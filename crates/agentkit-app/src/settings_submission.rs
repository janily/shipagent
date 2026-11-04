use super::connection::{connection_record, setup, test_params};
use super::*;
use agentkit_protocol::generated as g;
impl WorkbenchController {
    pub(super) fn submit_editor(
        &mut self,
        api: WorkbenchApi,
        editor: Entity<ConnectionEditor>,
        values: ConnectionValues,
        test: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let existing = self
            .app_state
            .read(cx)
            .connections
            .iter()
            .find(|c| c.slug == values.slug)
            .cloned();
        if values.existing != existing.is_some() {
            editor.update(cx, |e, cx| {
                e.status(
                    false,
                    Some(agentkit_ui::t!("ak.settings.stale_editor")),
                    None,
                    cx,
                )
            });
            return;
        }
        let record = match connection_record(&values, existing.as_ref()) {
            Ok(r) => r,
            Err(e) => {
                editor.update(cx, |editor, cx| {
                    editor.status(false, Some(e.to_string()), None, cx)
                });
                return;
            }
        };
        self.settings_pending = true;
        self.settings_status(true, None, None, cx);
        let epoch = self.epoch;
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let result = async {
                    if record.auth_type == g::LlmConnectionAuthType::Oauth {
                        if test {
                            api.test_connection(&record.slug).await?;
                        } else {
                            api.save_connection_metadata(&record).await?;
                        }
                    } else if test {
                        api.test_connection_draft(&test_params(&values, &record))
                            .await?;
                    } else {
                        api.save_connection(&setup(&values, &record), &record)
                            .await?;
                    }
                    api.connections_generated().await
                }
                .await;
                let _ = weak.update_in(cx, |this, window, cx| {
                    if this.epoch != epoch {
                        return;
                    }
                    this.settings_pending = false;
                    this.settings_status(false, None, None, cx);
                    let current = this
                        .settings_editor
                        .as_ref()
                        .is_some_and(|e| e.entity_id() == editor.entity_id());
                    match result {
                        Ok(catalog) => {
                            this.app_state.update(cx, |a, cx| {
                                a.connections = catalog;
                                cx.notify();
                            });
                            if current {
                                if test {
                                    editor.update(cx, |e, cx| {
                                        e.status(
                                            false,
                                            None,
                                            Some(
                                                if record.auth_type
                                                    == g::LlmConnectionAuthType::Oauth
                                                {
                                                    agentkit_ui::t!("ak.settings.auth_config_ok")
                                                } else {
                                                    agentkit_ui::t!("ak.settings.test_ok")
                                                },
                                            ),
                                            cx,
                                        )
                                    });
                                } else {
                                    editor.update(cx, |e, cx| e.clear_key(window, cx));
                                    window.close_sheet(cx);
                                    this.settings_status(
                                        false,
                                        None,
                                        Some(agentkit_ui::t!("ak.settings.operation_ok")),
                                        cx,
                                    );
                                }
                            }
                        }
                        Err(error) => {
                            let mut message = error.to_string();
                            if let Some(key) = &values.credential {
                                use secrecy::ExposeSecret;
                                if !key.expose_secret().is_empty() {
                                    message = message.replace(key.expose_secret(), "[REDACTED]");
                                }
                            }
                            if current {
                                editor.update(cx, |e, cx| e.status(false, Some(message), None, cx));
                            } else {
                                this.settings_status(false, Some(message), None, cx);
                            }
                        }
                    }
                });
            }));
    }
}
