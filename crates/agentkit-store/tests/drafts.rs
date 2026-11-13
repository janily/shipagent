#![allow(clippy::unwrap_used)]
use agentkit_protocol::generated as g;
use agentkit_store::core::drafts::*;
fn file(path: &str) -> g::FileAttachment {
    g::FileAttachment {
        path: path.into(),
        name: "note.txt".into(),
        type_: g::FileAttachmentType::Text,
        mime_type: "text/plain".into(),
        size: 6,
        text: Some("中文".into()),
        base64: Some("5Lit5paH".into()),
        thumbnail_base64: None,
    }
}
#[test]
fn path_backed_and_clipboard_content_refs_match_upstream() {
    for p in [
        "/tmp/note.txt",
        "C:\\Users\\a\\note.txt",
        "C:/Users/a/note.txt",
        r"\\server\share\note.txt",
        r"\\?\C:\note.txt",
    ] {
        let r = to_ref(&file(p)).unwrap();
        assert!(r.content.is_none());
        assert_eq!(r.path, p);
    }
    let f = file("clipboard:identity");
    let r = to_ref(&f).unwrap();
    assert!(r.content.is_some());
    let restored = from_content(&r).unwrap();
    assert_eq!(restored.size, 6);
    assert_eq!(restored.text, Some("中文".into()));
    assert_eq!(restored.base64, f.base64);
    assert!(from_content(&to_ref(&file("/tmp/a")).unwrap()).is_err());
}
#[test]
fn huge_inline_content_and_invalid_saved_metadata_are_rejected() {
    let mut f = file("paste");
    f.base64 = Some("a".repeat((CONTENT_PERSIST_CAP + 1) * 4 / 3 + 4));
    assert!(to_ref(&f).is_err());
    let mut r = to_ref(&file("paste")).unwrap();
    r.content.as_mut().unwrap().size = f64::NAN;
    assert!(from_content(&r).is_err());
    assert!(!is_absolute("relative/a"));
    assert!(!is_absolute("C:relative"));
    assert!(!is_absolute(""));
}
