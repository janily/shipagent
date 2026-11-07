use super::*;
use gpui_kit::component::input::InputState;
/// Native connection editor hosted in a Sheet.
pub struct ConnectionEditor {
    pub(super) onboarding: bool,
    pub(super) tested: bool,
    pub(super) input_subscriptions: Vec<Subscription>,
    pub(super) slug: String,
    pub(super) generation: u64,
    pub(super) existing: bool,
    pub(super) oauth: bool,
    pub(super) kind: ConnectionKind,
    pub(super) provider: String,
    pub(super) providers: Vec<(String, String)>,
    pub(super) models: Vec<(String, String)>,
    pub(super) name: Entity<InputState>,
    pub(super) url: Entity<InputState>,
    pub(super) model: Entity<InputState>,
    pub(super) model_list: Entity<InputState>,
    pub(super) key: Entity<InputState>,
    pub(super) replacing: bool,
    pub(super) busy: bool,
    pub(super) error: Option<String>,
    pub(super) notice: Option<String>,
    pub(super) focus: FocusHandle,
}
impl EventEmitter<EditorIntent> for ConnectionEditor {}
impl ConnectionEditor {
    /// Build from safe metadata only; saved credentials are never fetched.
    pub fn new(
        slug: String,
        existing: Option<g::LlmConnectionWithStatus>,
        providers: Vec<(String, String)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let oauth = existing
            .as_ref()
            .is_some_and(|s| s.auth_type == g::LlmConnectionWithStatusAuthType::Oauth);
        let kind = existing
            .as_ref()
            .map_or(ConnectionKind::Preset, ConnectionKind::from_connection);
        let models: Vec<_> = existing
            .as_ref()
            .and_then(|s| s.models.as_ref())
            .into_iter()
            .flatten()
            .map(|m| match m {
                g::LlmConnectionWithStatusModelsItem::String(id) => (id.clone(), id.clone()),
                g::LlmConnectionWithStatusModelsItem::Object { id, name, .. } => {
                    (id.clone(), name.clone())
                }
            })
            .collect();
        let name = existing.as_ref().map(|s| s.name.as_str()).unwrap_or("");
        let url = existing
            .as_ref()
            .and_then(|s| s.base_url.as_deref())
            .unwrap_or("");
        let model = existing
            .as_ref()
            .and_then(|s| s.default_model.as_deref())
            .unwrap_or("");
        Self {
            onboarding: false,
            tested: false,
            input_subscriptions: Vec::new(),
            slug,
            generation: 0,
            existing: existing.is_some(),
            oauth,
            kind,
            provider: existing
                .as_ref()
                .and_then(|s| s.pi_auth_provider.clone())
                .unwrap_or_else(|| "anthropic".into()),
            providers,
            model_list: cx.new(|cx| {
                InputState::new(window, cx).default_value(
                    models
                        .iter()
                        .map(|m| m.0.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                )
            }),
            models,
            name: cx.new(|cx| InputState::new(window, cx).default_value(name)),
            url: cx.new(|cx| InputState::new(window, cx).default_value(url)),
            model: cx.new(|cx| InputState::new(window, cx).default_value(model)),
            key: cx.new(|cx| InputState::new(window, cx).masked(true)),
            replacing: existing.is_none(),
            busy: false,
            error: None,
            notice: None,
            focus: cx.focus_handle(),
        }
    }
    /// Capture the current preset request; non-preset forms never accept provider replies.
    pub fn catalog_request(&self) -> Option<(String, u64)> {
        (self.kind == ConnectionKind::Preset && !self.oauth)
            .then(|| (self.provider.clone(), self.generation))
    }
    /// Verify ownership of an asynchronous provider reply.
    pub fn accepts_catalog(&self, provider: &str, generation: u64) -> bool {
        self.catalog_request()
            .is_some_and(|(p, g)| p == provider && g == generation)
    }
    /// Focus the name field after the Sheet's own focus setup.
    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.name.update(cx, |s, cx| s.focus(window, cx));
    }
    /// Apply only the requested provider's current model list.
    pub fn models(
        &mut self,
        provider: &str,
        generation: u64,
        models: Vec<(String, String)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.accepts_catalog(provider, generation) {
            return;
        }
        if self.model.read(cx).value().is_empty()
            && let Some(m) = models.first()
        {
            self.model
                .update(cx, |s, cx| s.set_value(m.0.clone(), window, cx));
        }
        self.models = models;
        cx.notify();
    }
    /// Clear the key when closing or after a successful save.
    pub fn clear_key(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.key.update(cx, |s, cx| s.set_value("", window, cx));
    }
    /// Serialize operations and show their acknowledged result.
    pub fn status(
        &mut self,
        busy: bool,
        error: Option<String>,
        notice: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.busy = busy;
        self.error = error;
        self.notice = notice;
        cx.notify();
    }
}

#[cfg(feature = "e2e")]
impl ConnectionEditor {
    /// Fill the isolated fixture form through real InputState changes. No saved key is read.
    pub fn e2e_endpoint(&mut self, url: &str, window: &mut Window, cx: &mut Context<Self>) {
        for (input, value) in [
            (&self.name, "e2e-connection"),
            (&self.url, url),
            (&self.model, "fixture-model"),
            (&self.model_list, "fixture-model"),
            (&self.key, "fixture-e2e-only"),
        ] {
            input.update(cx, |s, cx| s.set_value(value, window, cx));
        }
    }
    /// The same guarded submission used by the native Test/Save buttons.
    pub fn e2e_submit(&mut self, test: bool, cx: &mut Context<Self>) {
        self.submit(test, cx);
    }
    /// Non-secret form state: tested, busy, error visible.
    pub fn e2e_state(&self) -> (bool, bool, bool) {
        (self.tested, self.busy, self.error.is_some())
    }
}
