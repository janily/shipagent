use super::editor::*;
use super::*;
use gpui_kit::component::{button::Button, menu::DropdownMenu, *};
use serde::Deserialize;
#[derive(Clone, PartialEq, Deserialize, Action)]
#[action(namespace=settings,no_json)]
pub(super) struct ChooseProvider {
    pub id: String,
}
#[derive(Clone, PartialEq, Deserialize, Action)]
#[action(namespace=settings,no_json)]
pub(super) struct ChooseModel {
    pub id: String,
}
impl ConnectionEditor {
    pub(super) fn kind_buttons(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .gap_2()
            .flex_wrap()
            .children(
                [
                    (ConnectionKind::Preset, t!("ak.settings.preset")),
                    (ConnectionKind::OpenAi, t!("ak.settings.openai")),
                    (ConnectionKind::Anthropic, t!("ak.settings.anthropic")),
                    (ConnectionKind::Ollama, t!("ak.settings.ollama")),
                ]
                .into_iter()
                .enumerate()
                .map(|(i, (kind, label))| {
                    Button::new(("connection-kind", i))
                        .small()
                        .label(label)
                        .selected(self.kind == kind)
                        .disabled(self.busy || self.existing)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if this.kind == kind {
                                return;
                            }
                            this.generation = this.generation.wrapping_add(1);
                            this.kind = kind;
                            this.models.clear();
                            this.model.update(cx, |s, cx| s.set_value("", window, cx));
                            this.model_list
                                .update(cx, |s, cx| s.set_value("", window, cx));
                            this.key.update(cx, |s, cx| s.set_value("", window, cx));
                            this.url.update(cx, |s, cx| {
                                s.set_value(
                                    if kind == ConnectionKind::Ollama {
                                        "http://127.0.0.1:11434/v1"
                                    } else {
                                        ""
                                    },
                                    window,
                                    cx,
                                )
                            });
                            if kind == ConnectionKind::Preset {
                                cx.emit(EditorIntent::Provider {
                                    provider: this.provider.clone(),
                                    generation: this.generation,
                                });
                            }
                            cx.notify();
                        }))
                }),
            )
            .into_any_element()
    }
    pub(super) fn provider_menu(&self) -> impl IntoElement {
        let providers = self.providers.clone();
        let selected = self.provider.clone();
        let focus = self.focus.clone();
        let label = providers
            .iter()
            .find(|p| p.0 == selected)
            .map(|p| p.1.clone())
            .unwrap_or_else(|| selected.clone());
        Button::new("connection-provider")
            .outline()
            .label(label)
            .dropdown_caret(true)
            .disabled(self.busy || self.existing)
            .dropdown_menu_with_anchor(Anchor::TopLeft, move |mut m, _, _| {
                m = m.action_context(focus.clone());
                for (id, name) in &providers {
                    m = m.menu_with_check(
                        name.clone(),
                        id == &selected,
                        Box::new(ChooseProvider { id: id.clone() }),
                    );
                }
                m
            })
    }
    pub(super) fn model_menu(&self, cx: &App) -> impl IntoElement {
        let models = self.models.clone();
        let selected = self.model.read(cx).value().to_string();
        let focus = self.focus.clone();
        Button::new("connection-model-menu")
            .outline()
            .label(t!("ak.settings.choose_model"))
            .dropdown_caret(true)
            .disabled(self.busy || models.is_empty())
            .dropdown_menu_with_anchor(Anchor::TopLeft, move |mut m, _, _| {
                m = m.action_context(focus.clone());
                for (id, name) in &models {
                    m = m.menu_with_check(
                        name.clone(),
                        id == &selected,
                        Box::new(ChooseModel { id: id.clone() }),
                    );
                }
                m
            })
    }
}
