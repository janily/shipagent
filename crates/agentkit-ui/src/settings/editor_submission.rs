use super::*;
use secrecy::SecretString;
impl ConnectionEditor {
    pub(super) fn submit(&mut self, test: bool, cx: &mut Context<Self>) {
        if self.busy || (!test && self.onboarding && !self.tested) {
            return;
        }
        let model = self.model.read(cx).value().trim().to_string();
        let mut models: Vec<String> = self
            .model_list
            .read(cx)
            .value()
            .split([',', '\n'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        if models.is_empty() && self.kind == ConnectionKind::Preset {
            models = self.models.iter().map(|m| m.0.clone()).collect();
        }
        if !model.is_empty() && !models.contains(&model) {
            models.push(model.clone());
        }
        models.sort();
        models.dedup();
        let key = self.key.read(cx).value();
        let credential = if self.replacing && !key.is_empty() {
            Some(SecretString::from(key.to_string()))
        } else {
            None
        };
        let values = ConnectionValues {
            slug: self.slug.clone(),
            name: self.name.read(cx).value().trim().into(),
            kind: self.kind,
            provider: self.provider.clone(),
            url: self.url.read(cx).value().trim().into(),
            models,
            model,
            credential,
            existing: self.existing,
        };
        if values.name.is_empty()
            || values.model.is_empty()
            || values.kind != ConnectionKind::Preset && values.url.is_empty()
        {
            self.error = Some(t!("ak.settings.required"));
            cx.notify();
            return;
        }
        self.busy = true;
        self.error = None;
        self.notice = None;
        cx.emit(if test {
            EditorIntent::Test(values)
        } else {
            EditorIntent::Save(values)
        });
        cx.notify();
    }
}
