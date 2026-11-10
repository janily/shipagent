#![allow(clippy::unwrap_used)]
use super::*;
use std::prelude::v1::test;
#[test]
fn missing_and_corrupt_references_survive_partial_restore() {
    let temp = tempfile::tempdir().unwrap();
    let good = temp.path().join("readable.txt");
    std::fs::write(&good, "草稿文件").unwrap();
    let refs = vec![
        g::SessionDraftAttachmentsItem {
            path: good.to_string_lossy().into(),
            name: "readable.txt".into(),
            content: None,
        },
        g::SessionDraftAttachmentsItem {
            path: temp.path().join("missing.txt").to_string_lossy().into(),
            name: "missing.txt".into(),
            content: None,
        },
        g::SessionDraftAttachmentsItem {
            path: "clipboard:broken".into(),
            name: "bad.png".into(),
            content: Some(g::SessionDraftAttachmentsItemContent {
                type_: g::SessionDraftAttachmentsItemContentType::Image,
                mime_type: "image/png".into(),
                size: 3.,
                base64: Some("!!!".into()),
                text: None,
                thumbnail_base64: None,
            }),
        },
    ];
    let (files, failed) = restore(refs.clone());
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].file.text.as_deref(), Some("草稿文件"));
    assert_eq!(failed.len(), 2);
    assert_eq!(failed[0].reference.path, refs[1].path);
    assert_eq!(
        failed[1]
            .reference
            .content
            .as_ref()
            .unwrap()
            .base64
            .as_deref(),
        Some("!!!")
    );
    let wire = Snapshot {
        text: "new text".into(),
        files: files.into_iter().map(|f| f.file).collect(),
        references: failed.into_iter().map(|r| r.reference).collect(),
    }
    .to_wire()
    .unwrap();
    assert_eq!(wire.text, "new text");
    assert_eq!(wire.attachments.unwrap().len(), 3);
}
