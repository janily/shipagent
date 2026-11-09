use super::*;
use gpui_kit::component::{
    input::{Input, InputEvent, InputState},
    switch::Switch,
    *,
};
use secrecy::{ExposeSecret, SecretString};

/// Proxy form values deliberately have no Debug representation.
#[derive(Clone, Default)]
pub struct ProxyValues {
    /// Enable custom proxy routing.
    pub enabled: bool,
    /// Potentially credential-bearing HTTP proxy URL.
    pub http_proxy: SecretString,
    /// Potentially credential-bearing HTTPS proxy URL.
    pub https_proxy: SecretString,
    /// Comma-separated bypass rules.
    pub no_proxy: String,
}
impl ProxyValues {
    fn same_as(&self, other: &Self) -> bool {
        self.enabled == other.enabled
            && self.http_proxy.expose_secret() == other.http_proxy.expose_secret()
            && self.https_proxy.expose_secret() == other.https_proxy.expose_secret()
            && self.no_proxy == other.no_proxy
    }
}
pub(super) struct ProxyForm {
    http: Entity<InputState>,
    https: Entity<InputState>,
    bypass: Entity<InputState>,
    saved: ProxyValues,
    enabled: bool,
    available: bool,
    blocked: bool,
    pending: bool,
    error: bool,
    _subscriptions: Vec<Subscription>,
    focus: FocusHandle,
}
impl EventEmitter<ProxyValues> for ProxyForm {}
impl ProxyForm {
    pub fn new(
        values: ProxyValues,
        available: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let http = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .default_value(values.http_proxy.expose_secret())
        });
        let https = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .default_value(values.https_proxy.expose_secret())
        });
        let bypass = cx.new(|cx| InputState::new(window, cx).default_value(&values.no_proxy));
        let mut subscriptions = Vec::new();
        for input in [&http, &https, &bypass] {
            subscriptions.push(
                cx.subscribe_in(input, window, |this, _, event, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.propose(window, cx);
                    }
                }),
            );
        }
        let focus = cx.focus_handle();
        subscriptions.push(cx.on_focus_out(&focus, window, |this, _, window, cx| {
            this.propose(window, cx)
        }));
        Self {
            focus,
            enabled: values.enabled,
            saved: values,
            http,
            https,
            bypass,
            available,
            blocked: false,
            pending: false,
            error: false,
            _subscriptions: subscriptions,
        }
    }
    pub fn dirty(&self, cx: &App) -> bool {
        !self.values(cx).same_as(&self.saved)
    }
    pub fn blocked(&mut self, value: bool, cx: &mut Context<Self>) {
        if self.blocked != value {
            self.blocked = value;
            cx.notify();
        }
    }
    pub fn finished(&mut self, saved: bool, cx: &mut Context<Self>) {
        if saved {
            self.saved = self.values(cx);
        }
        self.pending = false;
        cx.notify();
    }
    fn values(&self, cx: &App) -> ProxyValues {
        ProxyValues {
            enabled: self.enabled,
            http_proxy: self.http.read(cx).value().trim().to_string().into(),
            https_proxy: self.https.read(cx).value().trim().to_string().into(),
            no_proxy: self.bypass.read(cx).value().trim().into(),
        }
    }
    fn propose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.available || self.blocked || self.pending {
            return;
        }
        if [&self.http, &self.https, &self.bypass]
            .iter()
            .any(|i| i.update(cx, |s, cx| s.marked_text_range(window, cx).is_some()))
        {
            return;
        }
        let values = self.values(cx);
        if values.same_as(&self.saved) {
            return;
        }
        // Detailed URL/credential validation belongs to the controller and backend.
        self.error = [
            values.http_proxy.expose_secret(),
            values.https_proxy.expose_secret(),
        ]
        .iter()
        .any(|s| !s.is_empty() && !(s.starts_with("http://") || s.starts_with("https://")));
        if !self.error {
            self.pending = true;
            cx.emit(values);
        }
        cx.notify();
    }
}
impl Render for ProxyForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let disabled = !self.available || self.blocked || self.pending;
        v_flex()
            .id("settings-proxy-form")
            .track_focus(&self.focus)
            .test_support()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("ak.settings.proxy_hint")),
            )
            .when(!self.available, |v| v.child(t!("ak.settings.proxy_remote")))
            .child(
                Switch::new("settings-proxy-enabled")
                    .label(t!("ak.settings.proxy_enabled"))
                    .checked(self.enabled)
                    .disabled(disabled)
                    .on_click(cx.listener(|s, value, window, cx| {
                        s.enabled = *value;
                        s.propose(window, cx);
                    })),
            )
            .child(t!("ak.settings.http_proxy"))
            .child(
                Input::new(&self.http)
                    .id("settings-http-proxy")
                    .aria_label(t!("ak.settings.http_proxy"))
                    .mask_toggle()
                    .disabled(disabled),
            )
            .child(t!("ak.settings.https_proxy"))
            .child(
                Input::new(&self.https)
                    .id("settings-https-proxy")
                    .aria_label(t!("ak.settings.https_proxy"))
                    .mask_toggle()
                    .disabled(disabled),
            )
            .child(t!("ak.settings.no_proxy"))
            .child(
                Input::new(&self.bypass)
                    .id("settings-no-proxy")
                    .aria_label(t!("ak.settings.no_proxy"))
                    .disabled(disabled),
            )
            .when(self.error, |v| {
                v.child(
                    div()
                        .id("settings-proxy-error")
                        .test_support()
                        .role(Role::Alert)
                        .aria_label(t!("ak.settings.proxy_invalid"))
                        .text_color(cx.theme().danger)
                        .child(t!("ak.settings.proxy_invalid")),
                )
            })
    }
}
