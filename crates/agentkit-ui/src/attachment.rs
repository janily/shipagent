//! Native attachment presentation shared by drafts and canonical messages.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use gpui_kit::component::attachment::{
    Attachment, AttachmentContent, AttachmentMedia, AttachmentTitle,
};
use gpui_kit::*;
/// Only bounded PNG thumbnail bytes are decoded locally; never load an arbitrary remote/path source.
pub fn card(name: String, thumbnail: Option<&str>) -> Attachment {
    let card =
        Attachment::new().content(AttachmentContent::new().title(AttachmentTitle::new(name)));
    if let Some(bytes) = thumbnail
        .filter(|s| s.len() < 2 * 1024 * 1024)
        .and_then(|s| STANDARD.decode(s).ok())
    {
        card.media(
            AttachmentMedia::new().src(std::sync::Arc::new(Image::from_bytes(
                ImageFormat::Png,
                bytes,
            ))),
        )
    } else {
        card
    }
}
