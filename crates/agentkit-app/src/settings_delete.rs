use super::*;
impl WorkbenchController {
    pub(super) fn confirm_connection_delete(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use gpui_kit::component::{
            button::{Button, ButtonVariants},
            dialog::DialogFooter,
        };
        let Some(connection) = self
            .app_state
            .read(cx)
            .connections
            .iter()
            .find(|c| c.slug == id)
        else {
            return;
        };
        let name = connection.name.clone();
        let weak = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let weak = weak.clone();
            let id = id.clone();
            dialog
                .title(agentkit_ui::t!("ak.settings.delete"))
                .child(agentkit_ui::t!(
                    "ak.settings.delete_confirm",
                    name = name.as_str()
                ))
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("connection-delete-cancel")
                                .outline()
                                .label(agentkit_ui::t!("common.cancel"))
                                .on_click(|_, w, cx| w.close_dialog(cx)),
                        )
                        .child(
                            Button::new("connection-delete-confirm")
                                .danger()
                                .label(agentkit_ui::t!("ak.settings.delete"))
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    let _ = weak.update(cx, |this, cx| {
                                        this.delete_saved_connection(id.clone(), window, cx)
                                    });
                                }),
                        ),
                )
        });
    }
    fn delete_saved_connection(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_pending {
            return;
        }
        let Some(api) = self.settings_api() else {
            return;
        };
        if !self
            .app_state
            .read(cx)
            .connections
            .iter()
            .any(|c| c.slug == id)
        {
            return;
        }
        self.settings_pending = true;
        self.settings_status(true, None, None, cx);
        let epoch = self.epoch;
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let result = async {
                    api.delete_connection(&id).await?;
                    api.connections_generated().await
                }
                .await;
                let _ = weak.update_in(cx, |this, _, cx| {
                    if this.epoch != epoch {
                        return;
                    }
                    this.settings_pending = false;
                    match result {
                        Ok(catalog) => {
                            this.app_state.update(cx, |a, cx| {
                                a.connections = catalog;
                                cx.notify();
                            });
                            this.settings_status(
                                false,
                                None,
                                Some(agentkit_ui::t!("ak.settings.operation_ok")),
                                cx,
                            );
                        }
                        Err(e) => this.settings_status(false, Some(e.to_string()), None, cx),
                    }
                });
            }));
    }
}
