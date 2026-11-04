use agentkit_store::core::turns::*;
use serde_json::Value;
pub(super) const MAX_LINES: usize = 2000;
pub(super) const MAX_BYTES: usize = 256 * 1024;
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Snippet {
    pub text: String,
    pub truncated: bool,
}
pub(super) fn snippet(text: &str, lines: usize, bytes: usize) -> Snippet {
    let mut end = text.len().min(bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    if let Some((position, _)) = text[..end]
        .bytes()
        .enumerate()
        .filter(|(_, c)| *c == b'\n')
        .nth(lines.saturating_sub(1))
    {
        end = position + 1;
    }
    Snippet {
        text: text[..end].into(),
        truncated: end < text.len(),
    }
}
struct Bounded {
    bytes: Vec<u8>,
    limit: usize,
    cut: bool,
}
impl std::io::Write for Bounded {
    fn write(&mut self, input: &[u8]) -> std::io::Result<usize> {
        let take = input.len().min(self.limit.saturating_sub(self.bytes.len()));
        self.bytes.extend_from_slice(&input[..take]);
        if take < input.len() {
            self.cut = true;
            return Err(std::io::ErrorKind::WriteZero.into());
        }
        Ok(take)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(super) fn json_preview(value: &Value, limit: usize) -> Snippet {
    let mut out = Bounded {
        bytes: Vec::new(),
        limit,
        cut: false,
    };
    let _ = serde_json::to_writer(&mut out, value);
    while std::str::from_utf8(&out.bytes).is_err() {
        out.bytes.pop();
    }
    Snippet {
        text: String::from_utf8_lossy(&out.bytes).into_owned(),
        truncated: out.cut,
    }
}
pub(super) fn language(path: &str) -> &'static str {
    match path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" | "mjs" => "javascript",
        "py" => "python",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" => "markdown",
        "sh" | "bash" => "bash",
        "html" => "html",
        "css" => "css",
        "c" | "h" => "c",
        "cpp" | "hpp" => "cpp",
        _ => "plaintext",
    }
}
#[derive(Clone, PartialEq, Eq)]
pub(super) enum Source {
    Patch {
        text: String,
        truncated: bool,
    },
    Change {
        path: String,
        before: String,
        after: String,
    },
    Code,
    Text,
}
#[derive(Clone, PartialEq, Eq)]
pub(super) struct Preview {
    pub id: String,
    pub fields: Vec<(String, Snippet)>,
    pub input_cut: bool,
    pub result: Snippet,
    pub source: Source,
    pub language: String,
    pub failed: bool,
}
fn string<'a>(
    input: Option<&'a serde_json::Map<String, Value>>,
    names: &[&str],
) -> Option<&'a str> {
    names.iter().find_map(|key| input?.get(*key)?.as_str())
}
impl Preview {
    pub fn new(activity: &Activity) -> Self {
        let input = activity.tool_input.as_ref();
        let path = string(input, &["file_path", "path", "filePath"]).unwrap_or("tool-result");
        let name = activity
            .tool_name
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let mut result = snippet(
            activity.content.as_deref().unwrap_or_default(),
            MAX_LINES,
            MAX_BYTES,
        );
        let mut source = if language(path) != "plaintext" {
            Source::Code
        } else {
            Source::Text
        };
        if matches!(name.as_str(), "edit" | "write" | "apply_patch") {
            let patch = string(input, &["patch", "diff"]);
            if let Some(patch) = patch {
                let bounded = snippet(patch, MAX_LINES, MAX_BYTES);
                result.truncated |= bounded.truncated;
                source = Source::Patch {
                    text: bounded.text,
                    truncated: bounded.truncated,
                };
            } else if let Some(changes) = input
                .and_then(|m| m.get("changes"))
                .and_then(Value::as_array)
            {
                let mut patches = String::new();
                let mut patch_cut = false;
                if changes.len() > 64 {
                    result.truncated = true;
                }
                for change in changes.iter().take(64) {
                    if let Some(diff) = change.get("diff").and_then(Value::as_str) {
                        let path = change
                            .get("path")
                            .and_then(Value::as_str)
                            .unwrap_or("tool-result")
                            .replace(['\n', '\r', '\t'], "_");
                        if diff.starts_with("@@") {
                            patches.push_str(&format!("--- a/{path}\n+++ b/{path}\n"));
                        }
                        let part =
                            snippet(diff, MAX_LINES, MAX_BYTES.saturating_sub(patches.len()));
                        patches.push_str(&part.text);
                        if !patches.ends_with('\n') {
                            patches.push('\n');
                        }
                        result.truncated |= part.truncated;
                        patch_cut |= part.truncated;
                        if patches.len() >= MAX_BYTES {
                            result.truncated = true;
                            break;
                        }
                    }
                }
                if !patches.is_empty() {
                    let bounded = snippet(&patches, MAX_LINES, MAX_BYTES);
                    result.truncated |= bounded.truncated;
                    source = Source::Patch {
                        text: bounded.text,
                        truncated: patch_cut || bounded.truncated,
                    };
                }
            } else {
                let old = string(input, &["old_string", "oldText", "old_text"]);
                let new = string(input, &["new_string", "newText", "new_text", "content"]);
                if let Some(new) = new.filter(|_| old.is_some() || name == "write") {
                    let old_text = old.unwrap_or_default();
                    let line_limit = if !old_text.is_empty() && !new.is_empty() {
                        MAX_LINES / 2
                    } else {
                        MAX_LINES
                    };
                    let old = snippet(old_text, line_limit, MAX_BYTES / 2);
                    let new = snippet(new, line_limit, MAX_BYTES / 2);
                    result.truncated |= old.truncated || new.truncated;
                    let path = path.replace(['\n', '\r', '\t'], "_");
                    source = Source::Change {
                        path,
                        before: old.text,
                        after: new.text,
                    };
                }
            }
        }
        Self {
            id: activity.id.clone(),
            fields: input
                .into_iter()
                .flat_map(|m| m.iter())
                .take(40)
                .map(|(k, v)| (k.clone(), json_preview(v, 320)))
                .collect(),
            input_cut: input.is_some_and(|m| m.len() > 40),
            result,
            source,
            language: language(path).into(),
            failed: activity.status == ActivityStatus::Error || activity.error.is_some(),
        }
    }
}
/// Thin metadata only: avoid cloning multi-megabyte output on a collapsed render.
pub(crate) fn grouped(activities: &[&Activity]) -> Vec<ActivityOrGroup> {
    let thin: Vec<_> = activities
        .iter()
        .map(|a| Activity {
            id: a.id.clone(),
            kind: a.kind,
            status: a.status,
            timestamp: a.timestamp,
            tool_name: a.tool_name.clone(),
            tool_use_id: a.tool_use_id.clone(),
            tool_input: None,
            content: if a.tool_name.as_deref() == Some("TaskOutput") {
                a.content
                    .as_deref()
                    .filter(|s| s.len() <= 32768)
                    .map(str::to_owned)
            } else if matches!(a.tool_name.as_deref(), Some("Task" | "Agent")) {
                a.content.as_deref().and_then(|s| {
                    s.find("agentId:")
                        .map(|i| s[i..].chars().take(256).collect())
                })
            } else {
                None
            },
            intent: None,
            message_id: a.message_id.clone(),
            annotations: None,
            display_name: None,
            tool_display_meta: None,
            error: None,
            parent_id: a.parent_id.clone(),
            depth: a.depth,
            status_type: a.status_type.clone(),
            task_id: a.task_id.clone(),
            shell_id: a.shell_id.clone(),
            elapsed_seconds: a.elapsed_seconds,
            is_background: a.is_background,
        })
        .collect();
    group_activities_by_parent(&thin)
}
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    #[test]
    fn bounds_are_lines_and_utf8_bytes_not_characters() {
        let text = (0..100_000).map(|i| format!("行{i}\n")).collect::<String>();
        let s = snippet(&text, MAX_LINES, MAX_BYTES);
        assert_eq!(s.text.lines().count(), 2000);
        assert!(s.truncated);
        assert_eq!(snippet("中文", 2, 4).text, "中");
        assert!(!snippet("a\nb\n", 2, 99).truncated);
    }
    #[test]
    fn long_json_fields_are_bounded_without_losing_utf8() {
        let s = json_preview(&serde_json::json!({"large":"中文".repeat(100000)}), 320);
        assert!(s.truncated);
        assert!(s.text.len() <= 320);
    }
    #[test]
    fn pi_and_claude_edits_preserve_before_and_after() {
        for (old, new) in [("oldText", "newText"), ("old_string", "new_string")] {
            let a:Activity=serde_json::from_value(serde_json::json!({"id":"edit","type":"tool","status":"completed","timestamp":1,"toolName":"Edit","toolInput":{old:"旧\n",new:"新\n","path":"a.rs"}})).unwrap();
            let p = Preview::new(&a);
            assert!(
                matches!(p.source,Source::Change{before,after,..} if before=="旧\n"&&after=="新\n")
            );
        }
    }
}
