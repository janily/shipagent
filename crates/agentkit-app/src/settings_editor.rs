use super::*;
use agentkit_protocol::generated as g;
use gpui_kit::component::Placement;
impl WorkbenchController {
    pub(super) fn open_connection_editor(
        &mut self,
        id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let existing = id.as_ref().and_then(|id| {
            self.app_state
                .read(cx)
                .connections
                .iter()
                .find(|c| &c.slug == id)
                .cloned()
        });
        if id.is_some() && existing.is_none() {
            return;
        }
        let fetch_provider = existing.as_ref().is_none_or(|c| {
            c.provider_type == g::LlmConnectionWithStatusProviderType::Pi
                && c.auth_type != g::LlmConnectionWithStatusAuthType::Oauth
        });
        let slug = id.unwrap_or_else(|| format!("workbench-{}", uuid::Uuid::new_v4()));
        let editor = cx.new(|cx| {
            ConnectionEditor::new(slug, existing, self.settings_providers.clone(), window, cx)
        });
        let epoch = self.epoch;
        self.settings_editor_subscription = Some(cx.subscribe_in(
            &editor,
            window,
            move |this, editor, intent: &EditorIntent, window, cx| {
                if this.epoch == epoch {
                    this.editor_action(editor.clone(), intent.clone(), window, cx);
                }
            },
        ));
        self.settings_editor = Some(editor.clone());
        let weak = cx.weak_entity();
        let closing_editor = editor.clone();
        let focus = editor.clone();
        window.open_sheet_at(Placement::Right, cx, move |sheet, _, _| {
            let weak = weak.clone();
            let closing_editor = closing_editor.clone();
            sheet
                .size(px(480.))
                .title(agentkit_ui::t!("ak.settings.edit_connection"))
                .child(editor.clone())
                .on_close(move |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        closing_editor.update(cx, |e, cx| e.clear_key(window, cx));
                        if this
                            .settings_editor
                            .as_ref()
                            .is_some_and(|e| e.entity_id() == closing_editor.entity_id())
                        {
                            this.settings_editor = None;
                            this.settings_editor_subscription = None;
                        }
                    });
                })
        });
        let provider_editor = focus.clone();
        window.defer(cx, move |window, cx| {
            focus.update(cx, |e, cx| e.focus(window, cx))
        });
        let request = provider_editor.read(cx).catalog_request();
        if fetch_provider && let Some((provider, generation)) = request {
            self.editor_action(
                provider_editor,
                EditorIntent::Provider {
                    provider,
                    generation,
                },
                window,
                cx,
            );
        }
    }
    pub(in crate::controller) fn editor_action(
        &mut self,
        editor: Entity<ConnectionEditor>,
        action: EditorIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(api) = self.settings_api(cx) else {
            editor.update(cx, |e, cx| {
                e.status(
                    false,
                    Some(agentkit_ui::t!("ak.connection_unavailable")),
                    None,
                    cx,
                )
            });
            return;
        };
        if let EditorIntent::Provider {
            provider,
            generation,
        } = action
        {
            let epoch = self.epoch;
            self.operations
                .push(cx.spawn_in(window, async move |weak, cx| {
                    let result = api.provider_models(&provider).await;
                    let _ = weak.update_in(cx, |this, window, cx| {
                        if this.epoch != epoch
                            || this
                                .settings_editor
                                .as_ref()
                                .is_none_or(|e| e.entity_id() != editor.entity_id())
                            || this.settings_pending
                            || !editor.read(cx).accepts_catalog(&provider, generation)
                        {
                            return;
                        }
                        match result {
                            Ok(models) => editor.update(cx, |e, cx| {
                                e.models(
                                    &provider,
                                    generation,
                                    models.models.into_iter().map(|m| (m.id, m.name)).collect(),
                                    window,
                                    cx,
                                )
                            }),
                            Err(e) => editor.update(cx, |editor, cx| {
                                editor.status(false, Some(e.to_string()), None, cx)
                            }),
                        }
                    });
                }));
            return;
        }
        if self.settings_pending {
            editor.update(cx, |e, cx| {
                e.status(
                    false,
                    Some(agentkit_ui::t!("ak.nav.operation_busy")),
                    None,
                    cx,
                )
            });
            return;
        }
        let (values, test) = match action {
            EditorIntent::Save(v) => (v, false),
            EditorIntent::Test(v) => (v, true),
            _ => return,
        };
        if self.view.read(cx).onboarding().is_some() {
            self.submit_onboarding(api, editor, values, test, window, cx);
        } else {
            self.submit_editor(api, editor, values, test, window, cx);
        }
    }
}
