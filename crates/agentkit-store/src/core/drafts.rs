//! Ported from mkagent v0.2.6 renderer/lib/drafts.ts; no filesystem or RPC work.
use agentkit_protocol::generated as g;
/// Upstream cap for a content-backed attachment, before base64 inflation.
pub const CONTENT_PERSIST_CAP: usize = 20 * 1024 * 1024;
/// Recognize POSIX, drive-rooted and Windows UNC paths on every host.
pub fn is_absolute(path: &str) -> bool {
    path.starts_with('/')
        || path.starts_with(r"\\")
        || (path.len() >= 3
            && path.as_bytes()[0].is_ascii_alphabetic()
            && path.as_bytes()[1] == b':'
            && matches!(path.as_bytes()[2], b'/' | b'\\'))
}
/// Store path-backed files by reference and synthetic clipboard files by content.
/// Oversized content is an explicit failure, never a silently omitted attachment.
pub fn to_ref(file: &g::FileAttachment) -> Result<g::SessionDraftAttachmentsItem, &'static str> {
    let content = if is_absolute(&file.path) {
        None
    } else {
        validate_content(file)?;
        Some(g::SessionDraftAttachmentsItemContent {
            base64: file.base64.clone(),
            mime_type: file.mime_type.clone(),
            size: file.size as f64,
            text: file.text.clone(),
            thumbnail_base64: file.thumbnail_base64.clone(),
            type_: match file.type_ {
                g::FileAttachmentType::Image => g::SessionDraftAttachmentsItemContentType::Image,
                g::FileAttachmentType::Pdf => g::SessionDraftAttachmentsItemContentType::Pdf,
                g::FileAttachmentType::Text => g::SessionDraftAttachmentsItemContentType::Text,
                g::FileAttachmentType::Office => g::SessionDraftAttachmentsItemContentType::Office,
                g::FileAttachmentType::Audio => g::SessionDraftAttachmentsItemContentType::Audio,
                g::FileAttachmentType::Unknown => {
                    g::SessionDraftAttachmentsItemContentType::Unknown
                }
            },
        })
    };
    Ok(g::SessionDraftAttachmentsItem {
        path: file.path.clone(),
        name: file.name.clone(),
        content,
    })
}
fn validate_content(file: &g::FileAttachment) -> Result<(), &'static str> {
    let estimated = file
        .base64
        .as_ref()
        .map(|s| s.len().saturating_mul(3) / 4)
        .unwrap_or(0)
        .max(
            file.text
                .as_ref()
                .map(|s| s.encode_utf16().count())
                .unwrap_or(0),
        );
    if estimated > CONTENT_PERSIST_CAP {
        Err("ak.draft.content_too_large")
    } else {
        Ok(())
    }
}
/// Reconstruct an inline reference. Path references require background file IO.
pub fn from_content(
    reference: &g::SessionDraftAttachmentsItem,
) -> Result<g::FileAttachment, &'static str> {
    let content = reference
        .content
        .as_ref()
        .ok_or("ak.draft.missing_content")?;
    if !content.size.is_finite()
        || content.size < 0.
        || content.size.fract() != 0.
        || content.size > CONTENT_PERSIST_CAP as f64
    {
        return Err("ak.draft.invalid_content");
    }
    let file = g::FileAttachment {
        path: reference.path.clone(),
        name: reference.name.clone(),
        base64: content.base64.clone(),
        text: content.text.clone(),
        thumbnail_base64: content.thumbnail_base64.clone(),
        mime_type: content.mime_type.clone(),
        size: content.size as u64,
        type_: match content.type_ {
            g::SessionDraftAttachmentsItemContentType::Image => g::FileAttachmentType::Image,
            g::SessionDraftAttachmentsItemContentType::Pdf => g::FileAttachmentType::Pdf,
            g::SessionDraftAttachmentsItemContentType::Text => g::FileAttachmentType::Text,
            g::SessionDraftAttachmentsItemContentType::Office => g::FileAttachmentType::Office,
            g::SessionDraftAttachmentsItemContentType::Audio => g::FileAttachmentType::Audio,
            g::SessionDraftAttachmentsItemContentType::Unknown => g::FileAttachmentType::Unknown,
        },
    };
    validate_content(&file)?;
    Ok(file)
}
/// A reference that could not be restored remains visible and persisted until removed.
#[derive(Clone, Debug)]
pub struct Unavailable {
    /// Stable native row identity.
    pub id: String,
    /// The original recoverable reference.
    pub reference: std::sync::Arc<g::SessionDraftAttachmentsItem>,
}
/// UI projection of the controller-owned draft service.
#[derive(Clone, Default)]
pub struct Status {
    /// Initial draft hydration in progress.
    pub loading: bool,
    /// Editing is frozen while normal quit drains pending writes.
    pub closing: bool,
    /// Latest change awaits a persistence acknowledgement.
    pub saving: bool,
    /// Actual load/save failure; retry remains available.
    pub error: Option<String>,
    /// Failed reads are never silently discarded or included in a send.
    pub unavailable: Vec<Unavailable>,
}
