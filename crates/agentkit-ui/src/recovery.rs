//! Native connection lifecycle; backend details and transport handles stay in the app.
use crate::{WorkbenchIntent, WorkbenchView, t};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::*,
    *,
};
/// User-visible connection state. Online is set only after authoritative refresh commits.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    /// Initial local backend startup.
    #[default]
    Starting,
    /// Transport and session cache are usable.
    Online,
    /// Socket retry countdown.
    Reconnecting {
        /// One-based transport retry.
        attempt: u32,
        /// Rounded-up seconds remaining.
        seconds: u64,
    },
    /// Local process is being replaced.
    Restarting {
        /// One-based supervision retry.
        attempt: u32,
    },
    /// Fetching authoritative state after handshake.
    Refreshing,
    /// Transport is usable but refreshing its cache failed.
    RefreshFailed,
    /// A terminal problem requires explicit intervention.
    Fatal {
        /// App-sanitized, localized explanation; never raw stderr or credentials.
        message: String,
    },
}
impl Phase {
    /// Stable diagnostic category without user content.
    pub fn category(&self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Online => "online",
            Self::Reconnecting { .. } => "reconnecting",
            Self::Restarting { .. } => "restarting",
            Self::Refreshing => "refreshing",
            Self::RefreshFailed => "refresh_failed",
            Self::Fatal { .. } => "fatal",
        }
    }
    /// Localized status text.
    pub fn label(&self) -> String {
        match self {
            Self::Starting => t!("ak.starting"),
            Self::Online => t!("ak.connected"),
            Self::Reconnecting { attempt, seconds } => t!(
                "ak.recovery.reconnecting",
                attempt = attempt,
                seconds = seconds
            ),
            Self::Restarting { attempt } => t!("ak.recovery.restarting", attempt = attempt),
            Self::Refreshing => t!("ak.recovery.refreshing"),
            Self::RefreshFailed => t!("ak.recovery.refresh_failed"),
            Self::Fatal { .. } => t!("ak.recovery.fatal"),
        }
    }
}
impl WorkbenchView {
    /// Observe lifecycle without replacing readable transcript content or local drafts.
    pub fn connection_state(&self) -> &Phase {
        &self.connection
    }
    /// Whether connection-dependent commands may run.
    pub fn connection_ready(&self) -> bool {
        self.ready && self.connection == Phase::Online
    }
    /// Apply a controller-owned state transition.
    pub fn set_connection_state(
        &mut self,
        state: Phase,
        can_restart: bool,
        cx: &mut Context<Self>,
    ) {
        if self.connection == state
            && self.can_restart == can_restart
            && self.ready == (state == Phase::Online)
        {
            return;
        }
        self.ready = state == Phase::Online;
        self.status_key = if state == Phase::Starting {
            "ak.starting"
        } else if self.ready {
            "ak.connected"
        } else {
            "ak.disconnected"
        };
        self.connection = state;
        self.can_restart = can_restart;
        if !self.ready {
            self.permission_prompt.update(cx, |p, cx| p.suspend(cx));
            self.composer.update(cx, |c, cx| c.selection_finished(cx));
        } else {
            self.error = None;
            self.loading = false;
            self.loading_visible = false;
            self.session_error = None;
        }
        cx.notify();
    }
    pub(crate) fn fatal_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let Phase::Fatal { message } = &self.connection else {
            return div().into_any_element();
        };
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                TitleBar::new()
                    .h_10()
                    .on_close_window(cx.listener(|_, _, _, cx| cx.emit(WorkbenchIntent::Quit)))
                    .when_some(self.menu_bar.clone(), |t, m| {
                        t.child(div().w(px(300.)).h_full().child(m))
                    })
                    .child(div().flex_1().text_center().child(self.app_name.clone())),
            )
            .child(
                v_flex()
                    .id("fatal-page")
                    .test_support()
                    .role(Role::Alert)
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_4()
                    .child(
                        div()
                            .text_xl()
                            .font_semibold()
                            .child(t!("ak.recovery.fatal")),
                    )
                    .child(
                        div()
                            .id("fatal-reason")
                            .test_support()
                            .max_w(px(640.))
                            .child(message.clone()),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .child(
                                Button::new("fatal-retry")
                                    .primary()
                                    .label(t!("ak.retry"))
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(WorkbenchIntent::Reconnect)
                                    })),
                            )
                            .child(
                                Button::new("open-recovery-logs")
                                    .label(t!("ak.recovery.logs"))
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(WorkbenchIntent::OpenRecoveryLogs)
                                    })),
                            )
                            .child(
                                Button::new("export-recovery")
                                    .label(t!("ak.recovery.export"))
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(WorkbenchIntent::ExportRecovery)
                                    })),
                            ),
                    )
                    .when_some(self.error.clone(), |v, e| {
                        v.child(div().text_color(cx.theme().danger).child(e))
                    }),
            )
            .into_any_element()
    }
    pub(crate) fn connection_banner(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if matches!(
            self.connection,
            Phase::Online | Phase::Starting | Phase::Fatal { .. }
        ) {
            return None;
        }
        Some(
            h_flex()
                .id("connection-banner")
                .test_support()
                .role(Role::Status)
                .px_4()
                .py_2()
                .gap_3()
                .bg(cx.theme().muted)
                .child(
                    div()
                        .flex_1()
                        .text_color(cx.theme().warning)
                        .child(self.connection.label()),
                )
                .when(!matches!(self.connection, Phase::Refreshing), |v| {
                    v.child(
                        Button::new("retry-connection")
                            .small()
                            .label(t!("ak.recovery.retry_now"))
                            .on_click(
                                cx.listener(|_, _, _, cx| cx.emit(WorkbenchIntent::Reconnect)),
                            ),
                    )
                })
                .into_any_element(),
        )
    }
    /// Mark the requested conversation load as failed; retry never creates a new session.
    pub fn session_load_failed(&mut self, cx: &mut Context<Self>) {
        self.loading = false;
        self.loading_visible = false;
        self.session_error = Some(t!("ak.recovery.session_failed"));
        cx.notify();
    }
}
