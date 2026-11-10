//! Local attachment drafts. IO and server conversion belong to the app.
use agentkit_protocol::generated as g;
use std::sync::Arc;
/// A stable draft identity is independent of the filename and server identity.
#[derive(Clone, Debug)]
pub struct Entry {
    /// Client-owned identity.
    pub id: String,
    /// Immutable bounded payload, shared across an in-flight snapshot.
    pub file: Arc<g::FileAttachment>,
}
/// Stays attached to its session across view switches and snapshot refreshes.
#[derive(Clone, Default)]
pub struct Draft {
    /// Unsent local files, in insertion order.
    pub files: Vec<Entry>,
    /// Background read count.
    pub reading: usize,
    /// Upload/send operation owns a snapshot, not subsequent edits.
    pub sending: bool,
    /// Individual local failures, rendered as conversation warning messages.
    pub warnings: Vec<String>,
}
impl Draft {
    /// Remove a user-selected file, including while an older snapshot is sending.
    pub fn remove(&mut self, id: &str) {
        self.files.retain(|e| e.id != id);
    }
    /// Only a successful ACK consumes the submitted successful identities.
    pub fn acknowledged(&mut self, ids: &[String], accepted: bool) {
        if accepted {
            self.files.retain(|e| !ids.contains(&e.id));
        }
        self.sending = false;
    }
    /// Bound presentation history; payloads are not retained by warnings.
    pub fn warn(&mut self, warning: String) {
        self.warnings.push(warning);
        if self.warnings.len() > 32 {
            self.warnings.remove(0);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_upload_and_new_edits_survive_older_ack() {
        let file = Arc::new(g::FileAttachment {
            name: "same.txt".into(),
            path: "/same.txt".into(),
            mime_type: "text/plain".into(),
            size: 1,
            type_: g::FileAttachmentType::Text,
            text: Some("a".into()),
            base64: None,
            thumbnail_base64: None,
        });
        let mut d = Draft::default();
        for id in ["success", "failed", "new"] {
            d.files.push(Entry {
                id: id.into(),
                file: file.clone(),
            });
        }
        d.acknowledged(&["success".into()], false);
        assert_eq!(d.files.len(), 3);
        d.acknowledged(&["success".into()], true);
        assert_eq!(
            d.files.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            ["failed", "new"]
        );
        d.remove("failed");
        assert_eq!(d.files[0].id, "new");
        for i in 0..40 {
            d.warn(i.to_string());
        }
        assert_eq!(d.warnings.len(), 32);
    }
}
