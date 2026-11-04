use super::model::*;
use gpui_kit::component::diff::DiffFile;
pub(super) fn prepare_diff(source: Source) -> Option<Vec<DiffFile>> {
    let mut truncated = false;
    let patch = match source {
        Source::Patch {
            text,
            truncated: cut,
        } => {
            truncated = cut;
            text
        }
        Source::Change {
            path,
            before,
            after,
        } => {
            if before == after {
                return Some(vec![DiffFile::unchanged(path, &after)]);
            }
            similar::TextDiff::configure()
                .timeout(std::time::Duration::from_millis(30))
                .diff_lines(&before, &after)
                .unified_diff()
                .header(&format!("a/{path}"), &format!("b/{path}"))
                .to_string()
        }
        _ => return None,
    };
    DiffFile::parse_with(
        &patch,
        gpui_kit::component::diff::DiffParseOptions::new().truncated_hunks(truncated),
    )
    .ok()
    .filter(|files| !files.is_empty())
}
#[cfg(test)]
mod tests {
    use super::*;
    use agentkit_store::core::turns::Activity;
    #[::core::prelude::v1::test]
    fn unrelated_large_result_does_not_relax_malformed_patch_validation()
    -> Result<(), serde_json::Error> {
        let activity: Activity = serde_json::from_value(
            serde_json::json!({"id":"e","type":"tool","status":"completed","timestamp":1,"toolName":"Edit","content":"result\n".repeat(3000),"toolInput":{"changes":[{"path":"a.rs","diff":"@@ -1,2 +1,2 @@\n-old\n+new\n"}]}}),
        )?;
        let preview = Preview::new(&activity);
        assert!(preview.result.truncated);
        assert!(matches!(
            &preview.source,
            Source::Patch {
                truncated: false,
                ..
            }
        ));
        assert!(prepare_diff(preview.source).is_none());
        Ok(())
    }
    #[::core::prelude::v1::test]
    fn codex_hunks_keep_exact_change_counts_without_inventing_context()
    -> Result<(), serde_json::Error> {
        let activity: Activity = serde_json::from_value(
            serde_json::json!({"id":"e","type":"tool","status":"completed","timestamp":1,"toolName":"Edit","toolInput":{"changes":[{"path":"a.rs","diff":"@@ -1 +1 @@\n-old\n+new\n"},{"path":"b.rs","diff":"@@ -1 +1 @@\n-before\n+after\n"}]}}),
        )?;
        let files = prepare_diff(Preview::new(&activity).source)
            .ok_or_else(|| serde::de::Error::custom("missing diff"))?;
        assert_eq!(files.len(), 2);
        assert_eq!(files.iter().map(DiffFile::additions).sum::<usize>(), 2);
        assert_eq!(files.iter().map(DiffFile::deletions).sum::<usize>(), 2);
        Ok(())
    }
}
