use super::*;
use agentkit_ui::settings::{ClaudeIntent, ClaudeLoginForm};
use gpui_kit::component::Placement;
#[path = "claude_login.rs"]
mod login;
impl WorkbenchController {
    pub(super) fn close_claude(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(cancel) = &self.claude_cancel {
            cancel.close();
        }
        self.claude_code.take();
        self.claude_subscription = None;
        if let Some(form) = self.claude_form.take() {
            form.update(cx, |v, cx| v.clear(window, cx));
            window.close_sheet(cx);
        }
    }
    pub(super) fn start_claude(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_pending || self.oauth_cancel.is_some() || self.claude_cancel.is_some() {
            return;
        }
        let Some(api) = self.settings_api(cx) else {
            self.claude_status(
                false,
                Some(agentkit_ui::t!("ak.connection_unavailable")),
                cx,
            );
            return;
        };
        let epoch = self.epoch;
        let transport = self.transport_epoch;
        let onboarding = self.view.read(cx).onboarding().is_some();
        let slug = format!("claude-max-{}", uuid::Uuid::new_v4().as_u128());
        let (cancel, cancelled) = async_channel::bounded(1);
        let (code, codes) = async_channel::bounded(1);
        let (ready_tx, ready_rx) = async_channel::bounded(1);
        let (tx, rx) = async_channel::bounded(1);
        self.claude_cancel = Some(cancel);
        self.claude_code = Some(code);
        self.settings_pending = true;
        self.claude_status(true, None, cx);
        let form = cx.new(|cx| ClaudeLoginForm::new(window, cx));
        self.claude_subscription = Some(cx.subscribe_in(
            &form,
            window,
            |this, _, intent: &ClaudeIntent, window, cx| match intent {
                ClaudeIntent::Cancel => this.close_claude(window, cx),
                ClaudeIntent::Submit(code) => {
                    if let Some(tx) = &this.claude_code {
                        let _ = tx.try_send(code.clone());
                    }
                }
            },
        ));
        self.claude_form = Some(form.clone());
        let weak = cx.weak_entity();
        let closing = form.clone();
        window.open_sheet_at(Placement::Right, cx, move |sheet, _, _| {
            let weak = weak.clone();
            let closing = closing.clone();
            sheet
                .size(px(480.))
                .title(agentkit_ui::t!("ak.claude.title"))
                .child(form.clone())
                .on_close(move |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        if this
                            .claude_form
                            .as_ref()
                            .is_some_and(|f| f.entity_id() == closing.entity_id())
                        {
                            if let Some(cancel) = &this.claude_cancel {
                                cancel.close();
                            }
                            this.claude_code.take();
                            this.claude_subscription = None;
                            this.claude_form.take();
                            closing.update(cx, |v, cx| v.clear(window, cx));
                        }
                    });
                })
        });
        let lease = cx.global::<SharedAppState>().1.clone();
        smol::spawn(async move {
            let result = login::login(
                login::Request {
                    api,
                    slug: slug.clone(),
                    onboarding,
                    cancel: cancelled,
                    code: codes,
                    ready: ready_tx,
                    lease,
                },
                |url| agentkit_capabilities::open_claude_url(url).map_err(|_| ()),
            )
            .await;
            let _ = tx.send((slug, result)).await;
        })
        .detach();
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                if ready_rx.recv().await.is_ok() {
                    let _ = weak.update_in(cx, |this, window, cx| {
                        if this.settings_operation_current(epoch, transport, cx)
                            && let Some(f) = &this.claude_form
                        {
                            f.update(cx, |v, cx| v.ready(window, cx));
                        }
                    });
                }
                let Ok((slug, result)) = rx.recv().await else {
                    return;
                };
                let _ = weak.update_in(cx, |this, window, cx| {
                    if !this.settings_operation_current(epoch, transport, cx) {
                        return;
                    }
                    let cancelled = this.claude_cancel.as_ref().is_none_or(|s| s.is_closed());
                    let result = if cancelled {
                        drop(result);
                        Err(login::LoginError::Cancelled)
                    } else {
                        result
                    };
                    this.close_claude(window, cx);
                    this.claude_cancel.take();
                    this.settings_pending = false;
                    match result {
                        Ok(pending) => {
                            let catalog = pending.accept();
                            let created = catalog
                                .iter()
                                .find(|c| c.slug == slug)
                                .map(|c| c.created_at)
                                .unwrap_or_default();
                            this.app_state.update(cx, |a, cx| {
                                a.connections = catalog;
                                cx.notify();
                            });
                            this.claude_status(false, None, cx);
                            if onboarding {
                                this.onboarding_saved = Some((slug, created));
                                this.preferences.update(|p| p.onboarding_deferred = false);
                                if let Some(v) = this.view.read(cx).onboarding() {
                                    v.update(cx, |v, cx| v.completed(cx));
                                }
                            }
                        }
                        Err(e) => {
                            use login::LoginError::*;
                            let message = match e {
                                Busy => agentkit_ui::t!("ak.claude.busy"),
                                Browser => agentkit_ui::t!("ak.oauth.browser_failed"),
                                Cancelled => agentkit_ui::t!("ak.oauth.cancelled"),
                                Timeout => agentkit_ui::t!("ak.oauth.timeout"),
                                Code => agentkit_ui::t!("ak.claude.code_failed"),
                                Backend => agentkit_ui::t!("ak.oauth.failed"),
                            };
                            this.claude_status(false, Some(message), cx);
                        }
                    }
                });
            }));
    }
    fn claude_status(&self, busy: bool, error: Option<String>, cx: &mut Context<Self>) {
        self.settings_status(
            busy,
            error.clone(),
            if error.is_some() {
                None
            } else {
                Some(if busy {
                    agentkit_ui::t!("ak.claude.waiting")
                } else {
                    agentkit_ui::t!("ak.oauth.connected")
                })
            },
            cx,
        );
        if let Some(v) = self.view.read(cx).onboarding() {
            v.update(cx, |v, cx| v.status(busy, error, cx));
        }
    }
}
