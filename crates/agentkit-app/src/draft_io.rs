//! Immutable snapshots and background attachment restoration for native drafts.
use agentkit_protocol::generated as g;
use agentkit_store::core::drafts;
use std::sync::Arc;
/// Large file bytes remain shared while typing; wire cloning happens only on the worker.
#[derive(Clone, Default)]
pub(crate) struct Snapshot {
    pub text: String,
    pub files: Vec<Arc<g::FileAttachment>>,
    pub references: Vec<Arc<g::SessionDraftAttachmentsItem>>,
}
impl Snapshot {
    pub fn to_wire(&self) -> Result<g::SessionDraft, &'static str> {
        let mut refs = self
            .files
            .iter()
            .map(|f| drafts::to_ref(f))
            .collect::<Result<Vec<_>, _>>()?;
        for r in &self.references {
            if r.content.is_some() {
                drafts::from_content(r)?;
            }
            refs.push(r.as_ref().clone());
        }
        Ok(g::SessionDraft {
            text: self.text.clone(),
            attachments: (!refs.is_empty()).then_some(refs),
        })
    }
}
fn restore_content(
    reference: &g::SessionDraftAttachmentsItem,
) -> anyhow::Result<g::FileAttachment> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let mut file = drafts::from_content(reference).map_err(anyhow::Error::msg)?;
    if let Some(base64) = &file.base64 {
        let bytes = STANDARD.decode(base64)?;
        if file.type_ == g::FileAttachmentType::Image {
            let validated =
                crate::files::from_bytes(file.path.clone(), format!("{}.png", file.name), bytes)?;
            file.thumbnail_base64 = validated.thumbnail_base64;
        }
    } else if file.type_ == g::FileAttachmentType::Image {
        anyhow::bail!("ak.draft.missing_content");
    }
    Ok(file)
}
/// A path failure remains a reference, alongside other successfully restored files.
pub(crate) fn restore(
    refs: Vec<g::SessionDraftAttachmentsItem>,
) -> (
    Vec<agentkit_store::core::attachments::Entry>,
    Vec<drafts::Unavailable>,
) {
    let mut files = Vec::new();
    let mut failed = Vec::new();
    for reference in refs {
        let id = uuid::Uuid::new_v4().to_string();
        let file = if reference.content.is_some() {
            restore_content(&reference)
        } else if drafts::is_absolute(&reference.path) {
            crate::files::read(std::path::Path::new(&reference.path))
        } else {
            Err(anyhow::anyhow!("ak.draft.invalid_content"))
        };
        match file {
            Ok(file) => files.push(agentkit_store::core::attachments::Entry {
                id,
                file: Arc::new(file),
            }),
            Err(_) => failed.push(drafts::Unavailable {
                id,
                reference: Arc::new(reference),
            }),
        }
    }
    (files, failed)
}

#[cfg(test)]
#[path = "draft_io_tests.rs"]
mod tests;
