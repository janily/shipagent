use super::*;
#[path = "chatgpt_login.rs"]
mod login;
impl WorkbenchController {
    pub(super) fn start_chatgpt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_pending || self.oauth_cancel.is_some() {
            return;
        }
        let Some(api) = self.settings_api(cx) else {
            self.chatgpt_status(
                false,
                Some(agentkit_ui::t!("ak.connection_unavailable")),
                cx,
            );
            return;
        };
        let slug = format!("chatgpt-plus-{}", uuid::Uuid::new_v4().as_u128());
        let onboarding = self.view.read(cx).onboarding().is_some();
        let epoch = self.epoch;
        let transport = self.transport_epoch;
        let (tx, rx) = async_channel::bounded(1);
        self.oauth_cancel = Some(tx);
        self.settings_pending = true;
        self.chatgpt_status(true, None, cx);
        let cancellation = rx.clone();
        let (result_tx, result_rx) = async_channel::bounded(1);
        let page = agentkit_ui::t!("ak.oauth.callback");
        // Detached worker observes cancellation even if the GPUI window/future is dropped.
        smol::spawn(async move {
            let result = login::login(api.clone(), &slug, rx, page, opener).await;
            let result = match result {
                Ok(()) => {
                    async {
                        if onboarding {
                            api.default_connection(&slug).await.map_err(|_| {
                                agentkit_capabilities::oauth_chatgpt::LoginError::Backend
                            })?;
                        }
                        let catalog = api.connections_generated().await.map_err(|_| {
                            agentkit_capabilities::oauth_chatgpt::LoginError::Backend
                        })?;
                        if !catalog.iter().any(|c| {
                            c.slug == slug
                                && c.is_authenticated
                                && (!onboarding || c.is_default == Some(true))
                        }) {
                            return Err(agentkit_capabilities::oauth_chatgpt::LoginError::Backend);
                        }
                        Ok((slug.clone(), catalog))
                    }
                    .await
                }
                Err(e) => Err(e),
            };
            let result = if cancellation.is_closed() || cancellation.try_recv().is_ok() {
                let _ = api.delete_connection(&slug).await;
                let _ = api.logout_chatgpt(&slug).await;
                Err(agentkit_capabilities::oauth_chatgpt::LoginError::Cancelled)
            } else {
                result
            };
            let _ = result_tx.send(result).await;
        })
        .detach();
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let Ok(result) = result_rx.recv().await else {
                    return;
                };
                let _ = weak.update_in(cx, |this, window, cx| {
                    if !this.settings_operation_current(epoch, transport, cx) {
                        return;
                    }
                    this.oauth_cancel.take();
                    this.settings_pending = false;
                    match result {
                        Ok((slug, catalog)) => {
                            let created = catalog
                                .iter()
                                .find(|c| c.slug == slug)
                                .map(|c| c.created_at)
                                .unwrap_or_default();
                            this.app_state.update(cx, |a, cx| {
                                a.connections = catalog;
                                cx.notify();
                            });
                            this.chatgpt_status(false, None, cx);
                            if onboarding {
                                this.onboarding_saved = Some((slug, created));
                                this.preferences.update(|p| p.onboarding_deferred = false);
                                if let Some(v) = this.view.read(cx).onboarding() {
                                    v.update(cx, |v, cx| v.completed(cx));
                                }
                            }
                            cx.activate(true);
                            window.activate_window();
                        }
                        Err(error) => {
                            use agentkit_capabilities::oauth_chatgpt::LoginError::*;
                            let message = match error {
                                Occupied => agentkit_ui::t!("ak.oauth.occupied"),
                                Cancelled => agentkit_ui::t!("ak.oauth.cancelled"),
                                Timeout => agentkit_ui::t!("ak.oauth.timeout"),
                                Denied => agentkit_ui::t!("ak.oauth.denied"),
                                Browser => agentkit_ui::t!("ak.oauth.browser_failed"),
                                Backend => agentkit_ui::t!("ak.oauth.failed"),
                            };
                            this.chatgpt_status(false, Some(message), cx);
                        }
                    }
                });
            }));
    }
    fn chatgpt_status(&self, active: bool, error: Option<String>, cx: &mut Context<Self>) {
        self.view.read(cx).settings().update(cx, |s, cx| {
            s.oauth_pending(active, cx);
            s.status(
                active,
                error.clone(),
                if error.is_some() {
                    None
                } else {
                    Some(if active {
                        agentkit_ui::t!("ak.oauth.waiting")
                    } else {
                        agentkit_ui::t!("ak.oauth.connected")
                    })
                },
                cx,
            );
        });
        if let Some(v) = self.view.read(cx).onboarding() {
            v.update(cx, |v, cx| {
                v.oauth_pending(active, cx);
                v.status(active, error, cx);
            });
        }
    }
}
fn opener(url: &str) -> Result<(), ()> {
    // This runs on the worker, never the GPUI foreground thread.
    agentkit_capabilities::open_browser_url(url).map_err(|_| ())
}
