use super::*;
use crate::controller::settings_actions::connection::{connection_record, setup, test_params};
use agentkit_ui::settings::ConnectionValues;
impl WorkbenchController {
    pub(in crate::controller) fn submit_onboarding(
        &mut self,
        api: WorkbenchApi,
        editor: Entity<ConnectionEditor>,
        values: ConnectionValues,
        test: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut record = match connection_record(&values, None) {
            Ok(v) => v,
            Err(e) => {
                editor.update(cx, |v, cx| v.status(false, Some(e.to_string()), None, cx));
                return;
            }
        };
        let owned = self.onboarding_saved.clone();
        if let Some((_, created)) = &owned {
            record.created_at = *created;
        }
        self.settings_pending = true;
        self.onboarding_status(true, None, cx);
        let epoch = self.epoch;
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let mut saved = false;
                let result: anyhow::Result<_> = async {
                    // Retest before saving as a controller-side guard against changed form values.
                    api.test_connection_draft(&test_params(&values, &record))
                        .await?;
                    if !test {
                        let catalog = api.connections_generated().await?;
                        anyhow::ensure!(
                            policy::can_save(owned.as_ref(), &record.slug, &catalog),
                            agentkit_ui::t!("ak.settings.stale_editor")
                        );
                    }
                    if !test {
                        let mut setup = setup(&values, &record);
                        setup.update_only = Some(owned.is_some());
                        api.save_connection(&setup, &record).await?;
                        saved = true;
                        api.default_connection(&record.slug).await?;
                    }
                    Ok(api.connections_generated().await?)
                }
                .await;
                let _ = weak.update_in(cx, |this, window, cx| {
                    if this.epoch != epoch {
                        return;
                    }
                    this.settings_pending = false;
                    if saved {
                        this.onboarding_saved = Some((record.slug.clone(), record.created_at));
                    }
                    this.onboarding_status(false, None, cx);
                    if this
                        .settings_editor
                        .as_ref()
                        .is_none_or(|e| e.entity_id() != editor.entity_id())
                    {
                        return;
                    }
                    match result {
                        Ok(connections) => {
                            if !test
                                && !connections.iter().any(|c| {
                                    c.slug == record.slug
                                        && c.is_authenticated
                                        && c.is_default == Some(true)
                                })
                            {
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
                            this.app_state.update(cx, |a, cx| {
                                a.connections = connections;
                                cx.notify();
                            });
                            if test {
                                editor.update(cx, |e, cx| e.test_passed(cx));
                            } else {
                                this.preferences.update(|p| p.onboarding_deferred = false);
                                this.clear_onboarding_editor(window, cx);
                                if let Some(v) = this.view.read(cx).onboarding() {
                                    v.update(cx, |v, cx| v.completed(cx));
                                }
                            }
                        }
                        Err(e) => {
                            let mut message = e.to_string();
                            if let Some(key) = &values.credential {
                                use secrecy::ExposeSecret;
                                if !key.expose_secret().is_empty() {
                                    message = message.replace(key.expose_secret(), "[REDACTED]");
                                }
                            }
                            editor.update(cx, |e, cx| e.status(false, Some(message), None, cx));
                        }
                    }
                });
            }));
    }
}
