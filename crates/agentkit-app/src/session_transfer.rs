use super::*;
use agentkit_client::api::types::ImportMode;
use agentkit_protocol::generated::{PermissionMode, SessionBundle};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    dialog::DialogFooter,
};
use std::{io::Read, path::Path};

const MAX_BUNDLE_BYTES: u64 = 16 * 1024 * 1024;

fn read_bundle(path: &Path) -> anyhow::Result<SessionBundle> {
    anyhow::ensure!(
        std::fs::metadata(path)?.is_file(),
        "Expected a regular file"
    );
    let mut data = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_BUNDLE_BYTES + 1)
        .read_to_end(&mut data)?;
    anyhow::ensure!(
        data.len() as u64 <= MAX_BUNDLE_BYTES,
        "Session bundle is too large"
    );
    let mut bundle: SessionBundle = serde_json::from_slice(&data)?;
    let header = &mut bundle.session.header;
    header.permission_mode = Some(PermissionMode::Ask);
    header.llm_connection = None;
    header.connection_locked = None;
    header.working_directory = None;
    header.hidden = None;
    Ok(bundle)
}

fn write_bundle(path: &Path, bundle: &SessionBundle) -> anyhow::Result<()> {
    let parent = path.parent().context("Missing export directory")?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer(&mut file, bundle)?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}

impl WorkbenchController {
    fn transfer_ready(&self, window: &mut Window, cx: &mut App) -> bool {
        !self.quitting.get()
            && !self.loading
            && !self.navigation_pending
            && self.view.read(cx).conversation_visible()
            && !window.has_active_dialog(cx)
            && !window.has_active_sheet(cx)
    }

    pub(super) fn branch_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.transfer_ready(window, cx) {
            return;
        }
        let Some(store) = &self.state else {
            return;
        };
        let model = &store.read(cx).model;
        if model.busy() || model.state.session.supports_branching == Some(false) {
            return;
        }
        let message = model.state.session.messages.iter().rev().find(|m| {
            m.role == agentkit_protocol::generated::MessageRole::User
                && m.hidden != Some(true)
                && m.is_pending != Some(true)
                && m.is_queued != Some(true)
        });
        if let Some(message) = message {
            self.handle(
                WorkbenchIntent::Chat(agentkit_ui::transcript::ChatIntent::BranchFrom(
                    message.id.clone(),
                )),
                window,
                cx,
            );
        }
    }

    pub(super) fn export_session(
        &mut self,
        id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.transfer_ready(window, cx) {
            return;
        }
        let Some(api) = self.settings_api(cx) else {
            return;
        };
        let Some(id) = id.or_else(|| {
            self.state
                .as_ref()
                .map(|s| s.read(cx).model.state.session.id.clone())
        }) else {
            return;
        };
        let epoch = self.epoch;
        let transport = self.transport_epoch;
        let picker = cx.prompt_for_new_path(&std::env::temp_dir(), Some("shipagent-session.json"));
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let result = async {
                    let Some(path) = picker.await?? else {
                        return Ok(());
                    };
                    let current = weak
                        .update_in(cx, |this, _, cx| {
                            this.settings_operation_current(epoch, transport, cx)
                        })
                        .unwrap_or(false);
                    if !current {
                        return Ok(());
                    }
                    let mut bundle = api.export_session_bundle(&id).await?;
                    bundle.session.header.llm_connection = None;
                    bundle.session.header.connection_locked = None;
                    cx.background_executor()
                        .spawn(async move { write_bundle(&path, &bundle) })
                        .await
                }
                .await;
                if result.is_err() {
                    let _ = weak.update_in(cx, |this, _, cx| {
                        this.transfer_error(agentkit_ui::t!("ak.transfer.export_failed"), cx);
                    });
                }
            }));
    }

    pub(super) fn choose_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.transfer_ready(window, cx) || self.settings_api(cx).is_none() {
            return;
        }
        let weak = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let fork = weak.clone();
            let move_session = weak.clone();
            dialog
                .title(agentkit_ui::t!("ak.menu.import"))
                .child(agentkit_ui::t!("ak.transfer.import_description"))
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("import-cancel")
                                .outline()
                                .label(agentkit_ui::t!("common.cancel"))
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("import-copy")
                                .outline()
                                .label(agentkit_ui::t!("ak.transfer.fork"))
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    let _ = fork.update(cx, |this, cx| {
                                        this.import_session(ImportMode::Fork, window, cx);
                                    });
                                }),
                        )
                        .child(
                            Button::new("import-move")
                                .primary()
                                .label(agentkit_ui::t!("ak.transfer.move"))
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    let _ = move_session.update(cx, |this, cx| {
                                        this.import_session(ImportMode::Move, window, cx);
                                    });
                                }),
                        ),
                )
        });
    }

    fn import_session(&mut self, mode: ImportMode, window: &mut Window, cx: &mut Context<Self>) {
        let Some(api) = self.settings_api(cx) else {
            return;
        };
        let Some(workspace) = self.workspace.as_ref().map(|w| w.id.clone()) else {
            return;
        };
        let epoch = self.epoch;
        let transport = self.transport_epoch;
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(agentkit_ui::t!("ak.menu.import").into()),
        });
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let result = async {
                    let paths = picker.await??;
                    let Some(path) = paths.and_then(|paths| paths.into_iter().next()) else {
                        return Ok(None);
                    };
                    let bundle = cx
                        .background_executor()
                        .spawn(async move { read_bundle(&path) })
                        .await?;
                    let current = weak
                        .update_in(cx, |this, window, cx| {
                            this.transfer_ready(window, cx)
                                && this.settings_operation_current(epoch, transport, cx)
                        })
                        .unwrap_or(false);
                    if !current {
                        return Ok(None);
                    }
                    Ok::<_, anyhow::Error>(Some(
                        api.import_session_bundle(&workspace, &bundle, mode).await?,
                    ))
                }
                .await;
                let _ = weak.update_in(cx, |this, window, cx| {
                    if !this.settings_operation_current(epoch, transport, cx) {
                        return;
                    }
                    match result {
                        Ok(Some(imported)) => {
                            this.handle(
                                WorkbenchIntent::OpenSession(imported.session_id),
                                window,
                                cx,
                            );
                            if imported
                                .warnings
                                .is_some_and(|warnings| !warnings.is_empty())
                            {
                                this.transfer_error(
                                    agentkit_ui::t!("ak.transfer.import_warning"),
                                    cx,
                                );
                            }
                        }
                        Ok(None) => {}
                        Err(_) => {
                            this.transfer_error(agentkit_ui::t!("ak.transfer.import_failed"), cx)
                        }
                    }
                });
            }));
    }

    fn transfer_error(&self, message: String, cx: &mut Context<Self>) {
        self.view
            .update(cx, |v, cx| v.show_operation_notice(message, cx));
    }
}
