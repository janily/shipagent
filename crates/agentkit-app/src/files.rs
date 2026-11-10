//! Bounded local attachment IO, executed on the background executor.
use agentkit_protocol::generated as g;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::{
    io::{Cursor, Read},
    path::Path,
};
/// Product limit applies before allocation and after a bounded read (including growing files).
pub const MAX_BYTES: u64 = 50 * 1024 * 1024;
/// Read one file without following device files or allocating from untrusted metadata.
pub fn read(path: &Path) -> anyhow::Result<g::FileAttachment> {
    anyhow::ensure!(
        std::fs::metadata(path)?.is_file(),
        "{}",
        agentkit_ui::t!("ak.attach.file_only")
    );
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let f = options.open(path)?;
    let meta = f.metadata()?;
    anyhow::ensure!(meta.is_file(), "{}", agentkit_ui::t!("ak.attach.file_only"));
    anyhow::ensure!(
        meta.len() <= MAX_BYTES,
        "{}",
        agentkit_ui::t!("ak.attach.too_large")
    );
    let mut bytes = Vec::new();
    f.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    from_bytes(
        path.to_string_lossy().into_owned(),
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        bytes,
    )
}
/// Clipboard images use the same decoder, bounds and wire representation as selected files.
pub fn from_bytes(path: String, name: String, bytes: Vec<u8>) -> anyhow::Result<g::FileAttachment> {
    anyhow::ensure!(
        bytes.len() as u64 <= MAX_BYTES,
        "{}",
        agentkit_ui::t!("ak.attach.too_large")
    );
    let mime = mime_guess::from_path(&name)
        .first_or_octet_stream()
        .essence_str()
        .to_owned();
    let type_ = if mime.starts_with("image/") {
        g::FileAttachmentType::Image
    } else if mime == "application/pdf" {
        g::FileAttachmentType::Pdf
    } else if mime.starts_with("audio/") {
        g::FileAttachmentType::Audio
    } else if mime.contains("officedocument")
        || mime.contains("msword")
        || mime.contains("ms-excel")
        || mime.contains("ms-powerpoint")
    {
        g::FileAttachmentType::Office
    } else if mime.starts_with("text/")
        || matches!(
            mime.as_str(),
            "application/json" | "application/xml" | "application/javascript"
        )
    {
        g::FileAttachmentType::Text
    } else {
        g::FileAttachmentType::Unknown
    };
    let text = if type_ == g::FileAttachmentType::Text {
        Some(String::from_utf8(bytes.clone())?)
    } else {
        None
    };
    let thumbnail_base64 = if type_ == g::FileAttachmentType::Image {
        let mut reader = image::ImageReader::new(Cursor::new(&bytes)).with_guessed_format()?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(128 * 1024 * 1024);
        limits.max_image_width = Some(16384);
        limits.max_image_height = Some(16384);
        reader.limits(limits);
        let thumb = reader.decode()?.thumbnail(256, 256);
        let mut png = Cursor::new(Vec::new());
        thumb.write_to(&mut png, image::ImageFormat::Png)?;
        Some(STANDARD.encode(png.into_inner()))
    } else {
        None
    };
    Ok(g::FileAttachment {
        path,
        name,
        mime_type: mime,
        size: bytes.len() as u64,
        type_,
        text,
        thumbnail_base64,
        base64: Some(STANDARD.encode(bytes)),
    })
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn text_binary_limits_directory_and_corrupt_images() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("context.txt");
        std::fs::write(&p, "中文").unwrap();
        let f = read(&p).unwrap();
        assert_eq!(f.text.as_deref(), Some("中文"));
        assert_eq!(f.size, 6);
        let large = t.path().join("large.dat");
        std::fs::File::create(&large)
            .unwrap()
            .set_len(MAX_BYTES + 1)
            .unwrap();
        assert!(read(&large).is_err());
        assert!(read(t.path()).is_err());
        assert!(from_bytes("clipboard".into(), "broken.png".into(), b"invalid".to_vec()).is_err());
        let b = from_bytes("/b".into(), "b.bin".into(), vec![0, 255]).unwrap();
        assert_eq!(b.type_, g::FileAttachmentType::Unknown);
        assert!(b.text.is_none());
    }
    #[test]
    fn image_payload_preserves_original_and_bounds_preview() {
        let original = image::DynamicImage::new_rgb8(1200, 600);
        let mut out = Cursor::new(Vec::new());
        original
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        let bytes = out.into_inner();
        let f = from_bytes("clipboard".into(), "paste.png".into(), bytes.clone()).unwrap();
        assert_eq!(STANDARD.decode(f.base64.unwrap()).unwrap(), bytes);
        let t = image::load_from_memory(&STANDARD.decode(f.thumbnail_base64.unwrap()).unwrap())
            .unwrap();
        assert_eq!((t.width(), t.height()), (256, 128));
    }
}

#[cfg(all(test, unix))]
#[test]
#[allow(clippy::unwrap_used)]
fn named_pipe_is_rejected_without_waiting_for_a_writer() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("pipe");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(read(&path).is_err());
    });
    assert!(rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap());
}
