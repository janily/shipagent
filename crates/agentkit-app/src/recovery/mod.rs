use super::*;
use agentkit_sidecar::SupervisorState;
use agentkit_ui::recovery::Phase;
mod diagnostics;
mod refresh;
#[derive(Default)]
pub(super) struct Recovery {
    revision: u64,
    force_refresh: bool,
    requested: Option<u64>,
    pub(super) pending: bool,
    pub(super) events: Vec<(DecodedSessionEvent, ReductionContext)>,
    pub(super) overflow: bool,
    pub(super) retry_load: Option<String>,
}
impl WorkbenchController {
    pub(super) fn set_phase(&self, phase: Phase, cx: &mut Context<Self>) {
        let restart = self
            .runtime
            .borrow()
            .as_ref()
            .is_some_and(|r| r.sidecar.is_some());
        self.app_state.update(cx, |s, cx| {
            let connected = phase == Phase::Online;
            if s.connected != connected {
                s.connected = connected;
                cx.notify();
            }
        });
        self.view
            .update(cx, |v, cx| v.set_connection_state(phase, restart, cx));
    }
    pub(super) fn start_monitor(
        &mut self,
        api: WorkbenchApi,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.monitor = Some(cx.spawn_in(window, async move |weak, cx| {
            let mut countdown: Option<(u32, std::time::Instant)> = None;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let snapshot = api.snapshot();
                let live = weak
                    .update_in(cx, |this, window, cx| {
                        if this.quitting.get() {
                            return false;
                        }
                        let supervisor = this
                            .runtime
                            .borrow()
                            .as_ref()
                            .and_then(|r| r.sidecar.as_ref())
                            .map(|s| (s.state(), s.last_error()));
                        if let Some((SupervisorState::Fatal, error)) = &supervisor {
                            this.close_recovery_editor(window, cx);
                            let message = error
                                .as_ref()
                                .map(sidecar_message)
                                .unwrap_or_else(|| agentkit_ui::t!("ak.recovery.backend_failed"));
                            this.interrupt_refresh(window, cx);
                            this.set_phase(Phase::Fatal { message }, cx);
                            return true;
                        }
                        if let Some((SupervisorState::Restarting { attempt }, _)) = supervisor {
                            this.interrupt_refresh(window, cx);
                            this.set_phase(Phase::Restarting { attempt }, cx);
                            return true;
                        }
                        match &snapshot.state {
                            ConnectionState::Connected => {
                                countdown = None;
                                if snapshot.recovery_id != this.transport_epoch
                                    && this.recovery.requested != Some(snapshot.recovery_id)
                                {
                                    this.refresh_connection(
                                        api.clone(),
                                        snapshot.clone(),
                                        window,
                                        cx,
                                    );
                                }
                            }
                            ConnectionState::Reconnecting {
                                attempt,
                                next_retry_ms,
                            } => {
                                if countdown.as_ref().is_none_or(|(a, _)| a != attempt) {
                                    countdown = Some((
                                        *attempt,
                                        std::time::Instant::now()
                                            + Duration::from_millis(*next_retry_ms),
                                    ));
                                }
                                let seconds = countdown.as_ref().map_or(0, |(_, until)| {
                                    until
                                        .saturating_duration_since(std::time::Instant::now())
                                        .as_millis()
                                        .div_ceil(1000) as u64
                                });
                                this.interrupt_refresh(window, cx);
                                this.set_phase(
                                    Phase::Reconnecting {
                                        attempt: *attempt,
                                        seconds,
                                    },
                                    cx,
                                );
                            }
                            ConnectionState::Fatal { .. } => {
                                this.close_recovery_editor(window, cx);
                                this.interrupt_refresh(window, cx);
                                this.set_phase(
                                    Phase::Fatal {
                                        message: agentkit_ui::t!("ak.recovery.transport_failed"),
                                    },
                                    cx,
                                );
                            }
                            ConnectionState::Disconnected => {
                                this.close_recovery_editor(window, cx);
                                this.interrupt_refresh(window, cx);
                                this.set_phase(
                                    Phase::Fatal {
                                        message: agentkit_ui::t!("ak.recovery.closed"),
                                    },
                                    cx,
                                );
                            }
                            _ => {
                                if !matches!(
                                    this.view.read(cx).connection_state(),
                                    Phase::Restarting { .. }
                                ) {
                                    this.set_phase(
                                        Phase::Reconnecting {
                                            attempt: countdown.as_ref().map_or(1, |(n, _)| *n),
                                            seconds: 0,
                                        },
                                        cx,
                                    );
                                }
                            }
                        }
                        true
                    })
                    .unwrap_or(false);
                if !live {
                    break;
                }
            }
        }));
    }
    fn close_recovery_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = self.settings_editor.take() {
            editor.update(cx, |e, cx| e.clear_key(window, cx));
            window.close_sheet(cx);
        }
        self.settings_editor_subscription = None;
    }
    fn interrupt_refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.recovery.pending {
            return;
        }
        self.recovery.pending = false;
        self.recovery.force_refresh |= self.recovery.overflow;
        let events = std::mem::take(&mut self.recovery.events);
        self.route(events, window, cx);
    }
    pub(super) fn retry_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let phase = self.view.read(cx).connection_state().clone();
        match phase {
            Phase::Reconnecting { .. } => {
                if let Some(api) = self.api() {
                    api.retry_now();
                }
            }
            Phase::Restarting { .. } => self.restart_backend(false, window, cx),
            Phase::RefreshFailed => {
                if let Some(api) = self.api() {
                    let snapshot = api.snapshot();
                    self.refresh_connection(api, snapshot, window, cx);
                }
            }
            Phase::Refreshing | Phase::Starting => {}
            _ => self.start(window, cx),
        }
    }
    pub(super) fn restart_backend(
        &mut self,
        confirm: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if confirm
            && (self.stores.values().any(|s| s.read(cx).model.busy())
                || self
                    .list
                    .read(cx)
                    .model
                    .sessions
                    .iter()
                    .any(|s| s.is_processing))
        {
            if window.has_active_dialog(cx) {
                return;
            }
            let weak = cx.weak_entity();
            window.open_alert_dialog(cx, move |alert, _, _| {
                let weak = weak.clone();
                alert
                    .title(agentkit_ui::t!("ak.recovery.restart"))
                    .description(agentkit_ui::t!("ak.recovery.restart_confirm"))
                    .on_ok(move |_, w, cx| {
                        let _ = weak.update(cx, |this, cx| this.restart_backend(false, w, cx));
                        true
                    })
            });
            return;
        }
        let request = self
            .runtime
            .borrow()
            .as_ref()
            .and_then(|r| r.sidecar.as_ref())
            .map(SidecarHandle::request_restart);
        if let Some(request) = request {
            self.close_recovery_editor(window, cx);
            self.save_draft(cx);
            self.set_phase(Phase::Restarting { attempt: 1 }, cx);
            self.operations.push(cx.spawn(async move |weak, cx| {
                if let Err(error) = request.await {
                    let _ = weak.update(cx, |this, cx| {
                        this.set_phase(
                            Phase::Fatal {
                                message: sidecar_message(&error),
                            },
                            cx,
                        )
                    });
                }
            }));
        }
    }
}

#[cfg(test)]
mod tests;
