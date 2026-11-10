use super::*;
use crate::AttachmentInput;
use gpui_kit::component::h_flex;
impl Composer {
    pub(super) fn has_attachments(&self, cx: &App) -> bool {
        self.store.as_ref().is_some_and(|s| {
            !s.read(cx).attachments.files.is_empty()
                || !s.read(cx).model.draft_attachments.is_empty()
        })
    }
    pub(super) fn attachments_busy(&self, cx: &App) -> bool {
        self.store.as_ref().is_some_and(|s| {
            s.read(cx).attachments.sending
                || s.read(cx).attachments.reading > 0
                || s.read(cx).draft_sync.loading
                || s.read(cx).draft_sync.closing
                || !s.read(cx).draft_sync.unavailable.is_empty()
        })
    }
    pub(super) fn pick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selectable() {
            return;
        }
        let Some(id) = self.session(cx).map(|s| s.id.clone()) else {
            return;
        };
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(t!("ak.attach.pick").into()),
        });
        cx.spawn_in(window, async move |weak, cx| match paths.await {
            Ok(Ok(Some(paths))) => {
                let _ = weak.update_in(cx, |_, _, cx| {
                    cx.emit(WorkbenchIntent::Attach {
                        session_id: id,
                        inputs: paths.into_iter().map(AttachmentInput::Path).collect(),
                    })
                });
            }
            Ok(Ok(None)) => {}
            _ => {
                let _ = weak.update_in(cx, |_, _, cx| {
                    cx.emit(WorkbenchIntent::Attach {
                        session_id: id,
                        inputs: Vec::new(),
                    })
                });
            }
        })
        .detach();
    }
    pub(super) fn paste(
        &mut self,
        _: &gpui_kit::component::input::Paste,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.input.read(cx).focus_handle(cx).is_focused(window)
            || !self.selectable()
            || self.composing(window, cx)
        {
            cx.propagate();
            return;
        }
        let Some(item) = cx.read_from_clipboard() else {
            cx.propagate();
            return;
        };
        let mut inputs = Vec::new();
        for entry in item.entries() {
            match entry {
                ClipboardEntry::ExternalPaths(paths) => {
                    inputs.extend(paths.0.iter().cloned().map(AttachmentInput::Path))
                }
                ClipboardEntry::Image(image) => inputs.push(AttachmentInput::Image {
                    name: format!("pasted.{}", image.format().extension()),
                    bytes: std::sync::Arc::from(image.bytes()),
                }),
                _ => {}
            }
        }
        if inputs.is_empty() {
            cx.propagate();
            return;
        }
        cx.stop_propagation();
        if let Some(session) = self.session(cx) {
            cx.emit(WorkbenchIntent::Attach {
                session_id: session.id.clone(),
                inputs,
            });
        }
    }
    pub(super) fn attachment_row(&self, cx: &Context<Self>) -> impl IntoElement {
        let mut row = h_flex().gap_2().flex_wrap();
        if let Some(store) = &self.store {
            let s = store.read(cx);
            for entry in &s.attachments.files {
                let id = entry.id.clone();
                let session_id = s.model.state.session.id.clone();
                row = row.child(
                    div()
                        .id(SharedString::from(format!("attachment-row:{}", entry.id)))
                        .test_support()
                        .child(
                            crate::attachment::card(
                                entry.file.name.clone(),
                                entry.file.thumbnail_base64.as_deref(),
                            )
                            .id(SharedString::from(format!("attachment:{}", entry.id)))
                            .tooltip(entry.file.name.clone())
                            .on_remove(cx.listener(
                                move |_, _, _, cx| {
                                    cx.emit(WorkbenchIntent::RemoveAttachment {
                                        session_id: session_id.clone(),
                                        id: id.clone(),
                                    })
                                },
                            )),
                        ),
                );
            }
            for entry in &s.model.draft_attachments {
                let id = format!("stored:{}", entry.id);
                let session_id = s.model.state.session.id.clone();
                row = row.child(
                    crate::attachment::card(entry.name.clone(), entry.thumbnail_base64.as_deref())
                        .id(SharedString::from(format!(
                            "stored-attachment:{}",
                            entry.id
                        )))
                        .on_remove(cx.listener(move |_, _, _, cx| {
                            cx.emit(WorkbenchIntent::RemoveAttachment {
                                session_id: session_id.clone(),
                                id: id.clone(),
                            })
                        })),
                );
            }
            for pending in &s.draft_sync.unavailable {
                let id = format!("draft-ref:{}", pending.id);
                let session_id = s.model.state.session.id.clone();
                row = row.child(
                    div()
                        .id(SharedString::from(format!("draft-ref-row:{}", pending.id)))
                        .test_support()
                        .child(
                            crate::attachment::card(pending.reference.name.clone(), None)
                                .id(SharedString::from(id.clone()))
                                .on_remove(cx.listener(move |_, _, _, cx| {
                                    cx.emit(WorkbenchIntent::RemoveAttachment {
                                        session_id: session_id.clone(),
                                        id: id.clone(),
                                    })
                                })),
                        ),
                );
            }
            if s.attachments.reading > 0 {
                row = row.child(t!("ak.attach.reading"));
            }
            if s.attachments.sending {
                row = row.child(t!("ak.attach.sending"));
            }
        }
        row
    }
}
