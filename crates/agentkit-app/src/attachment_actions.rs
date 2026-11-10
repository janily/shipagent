use super::*;
use agentkit_protocol::generated as g;
use agentkit_store::core::attachments::Entry;
impl WorkbenchController {
    pub(super) fn import_attachments(
        &mut self,
        id: String,
        inputs: Vec<agentkit_ui::AttachmentInput>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = self.stores.get(&id).cloned() else {
            return;
        };
        store.update(cx, |s, cx| {
            s.attachments.reading += 1;
            cx.notify();
        });
        let task = cx.background_executor().spawn(async move {
            inputs
                .into_iter()
                .map(|input| match input {
                    agentkit_ui::AttachmentInput::Path(path) => {
                        let name = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned();
                        (name, crate::files::read(&path))
                    }
                    agentkit_ui::AttachmentInput::Image { name, bytes } => {
                        let result = crate::files::from_bytes(
                            format!("clipboard:{}", uuid::Uuid::new_v4()),
                            name.clone(),
                            bytes.to_vec(),
                        );
                        (name, result)
                    }
                })
                .collect::<Vec<_>>()
        });
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let results = task.await;
                let _ = weak.update_in(cx, |this, _, cx| {
                    if this
                        .stores
                        .get(&id)
                        .is_none_or(|s| s.entity_id() != store.entity_id())
                    {
                        return;
                    }
                    store.update(cx, |s, cx| {
                        s.attachments.reading = s.attachments.reading.saturating_sub(1);
                        if results.is_empty() {
                            s.attachments
                                .warn(agentkit_ui::t!("ak.attach.picker_failed"));
                        }
                        for (name, result) in results {
                            match result {
                                Ok(file) => s.attachments.files.push(Entry {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    file: std::sync::Arc::new(file),
                                }),
                                Err(e) => s.attachments.warn(agentkit_ui::t!(
                                    "ak.attach.failed",
                                    name = name,
                                    error = e.to_string()
                                )),
                            }
                        }
                        cx.notify();
                    });
                });
            }));
    }
    pub(super) fn submit_attachments(
        &mut self,
        text: String,
        store: Entity<SessionStore>,
        api: WorkbenchApi,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if store.read(cx).attachments.sending
            || store.read(cx).attachments.reading > 0
            || store.read(cx).plan.pending.is_some()
            || store.read(cx).selection_pending
        {
            return;
        }
        self.save_draft(cx);
        let id = store.read(cx).model.state.session.id.clone();
        let revision = store.read(cx).model.draft_revision();
        let files = store.read(cx).attachments.files.clone();
        let mut previous = store.read(cx).model.draft_attachments.clone();
        store.update(cx, |s, cx| {
            s.attachments.sending = true;
            cx.notify();
        });
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let upload = api.clone();
                let session = id.clone();
                let mut batch = cx
                    .background_executor()
                    .spawn(async move { store_batch(&upload, &session, &files).await })
                    .await;
                let valid = weak
                    .update_in(cx, |this, _, cx| {
                        if this
                            .stores
                            .get(&id)
                            .is_none_or(|s| s.entity_id() != store.entity_id())
                        {
                            return false;
                        }
                        store.update(cx, |s, cx| {
                            for (name, e) in &batch.failures {
                                s.attachments.warn(agentkit_ui::t!(
                                    "ak.attach.failed",
                                    name = name,
                                    error = e
                                ));
                            }
                            cx.notify();
                        });
                        true
                    })
                    .unwrap_or(false);
                if !valid {
                    return;
                }
                previous.extend(std::mem::take(&mut batch.stored));
                if text.trim().is_empty() && previous.is_empty() {
                    let _ = weak.update_in(cx, |this, window, cx| {
                        store.update(cx, |s, cx| {
                            s.attachments.sending = false;
                            cx.notify();
                        });
                        this.finish_attachment_view(&store, &text, false, window, cx);
                    });
                    return;
                }
                let pending = weak
                    .update_in(cx, |_, _, cx| {
                        store.update(cx, |s, cx| {
                            // The controller owns the raw snapshot; the ordinary model may not
                            // adopt/clear newer draft text while this upload is finishing.
                            let p = s.model.begin_resend(
                                text.clone(),
                                previous.clone(),
                                &reduction_context(),
                            );
                            s.refresh_annotations();
                            cx.emit(
                                agentkit_store::core::session::SessionStoreEvent::TurnsChanged {
                                    from_turn: 0,
                                },
                            );
                            cx.notify();
                            p
                        })
                    })
                    .ok()
                    .flatten();
                let Some(pending) = pending else {
                    return;
                };
                let sent_ids = previous.iter().map(|a| a.id.clone()).collect::<Vec<_>>();
                let payload = std::mem::take(&mut batch.files);
                let sender = api.clone();
                let session = id.clone();
                let message = text.clone();
                let message_id = pending.id.clone();
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        sender
                            .send_prepared(&session, &message, &message_id, &payload, &previous)
                            .await
                    })
                    .await;
                let _ = weak.update_in(cx, |this, window, cx| {
                    if this
                        .stores
                        .get(&id)
                        .is_none_or(|s| s.entity_id() != store.entity_id())
                    {
                        return;
                    }
                    let accepted = result.as_ref().is_ok_and(|a| a.accepted)
                        || store.read(cx).model.send_confirmed(&pending.id);
                    let clear_text = accepted
                        && store.read(cx).model.draft_revision() == revision
                        && store.read(cx).model.draft.text == text;
                    store.update(cx, |s, cx| {
                        s.model.finish_send(&pending.id, accepted);
                        s.attachments.acknowledged(&batch.ids, accepted);
                        if clear_text {
                            s.model.set_draft(String::new());
                        }
                        if accepted {
                            let remaining = s
                                .model
                                .draft_attachments
                                .iter()
                                .filter(|a| !sent_ids.contains(&a.id))
                                .cloned()
                                .collect();
                            s.model.set_draft_attachments(remaining);
                        }
                        if !accepted {
                            s.attachments.warn(
                                result
                                    .as_ref()
                                    .err()
                                    .map(|e| e.to_string())
                                    .unwrap_or_else(|| agentkit_ui::t!("ak.send_rejected")),
                            );
                        }
                        cx.emit(
                            agentkit_store::core::session::SessionStoreEvent::TurnsChanged {
                                from_turn: 0,
                            },
                        );
                        cx.notify();
                    });
                    this.finish_attachment_view(&store, &text, clear_text, window, cx);
                });
            }));
    }
    fn finish_attachment_view(
        &mut self,
        store: &Entity<SessionStore>,
        text: &str,
        clear: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .state
            .as_ref()
            .is_some_and(|s| s.entity_id() == store.entity_id())
        {
            self.view
                .update(cx, |v, cx| v.submission_finished(text, clear, window, cx));
        }
    }
}
/// Successful payloads retain server conversion references, failures never abort neighbors.
#[derive(Default)]
struct StoredBatch {
    ids: Vec<String>,
    files: Vec<agentkit_client::api::types::PreparedAttachment>,
    stored: Vec<g::StoredAttachment>,
    failures: Vec<(String, String)>,
}
async fn store_batch(api: &WorkbenchApi, id: &str, entries: &[Entry]) -> StoredBatch {
    let mut batch = StoredBatch::default();
    for entry in entries {
        match api.store_attachment(id, &entry.file).await {
            Ok(mut stored) => {
                let mut file = (*entry.file).clone();
                file.mime_type = stored.mime_type.clone();
                if let Some(b64) = &stored.resized_base64 {
                    file.base64 = Some(b64.clone());
                }
                stored.resized_base64 = None;
                batch.ids.push(entry.id.clone());
                batch
                    .files
                    .push(agentkit_client::api::types::PreparedAttachment {
                        file,
                        stored_path: stored.stored_path.clone(),
                        markdown_path: stored.markdown_path.clone(),
                    });
                batch.stored.push(stored);
            }
            Err(e) => batch
                .failures
                .push((entry.file.name.clone(), e.to_string())),
        }
    }
    batch
}

#[cfg(test)]
#[path = "attachment_tests.rs"]
mod tests;
