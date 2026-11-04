use super::*;
use agentkit_protocol::generated as g;
use agentkit_ui::tools::{FullToolEditor, ToolContent, language_for_path};
use gpui_kit::component::WindowExt;
enum Document {
    Text(String),
    Json(serde_json::Value),
}
fn document(session: &g::Session, id: &str, part: &ToolContent) -> Option<(Document, String)> {
    let message = session
        .messages
        .iter()
        .find(|m| m.id == id && matches!(m.role, g::MessageRole::Tool | g::MessageRole::Plan))?;
    let input = message.tool_input.as_ref();
    let path = ["file_path", "path", "filePath"]
        .iter()
        .find_map(|k| input?.get(*k)?.as_str())
        .unwrap_or_default();
    let language = language_for_path(path).to_owned();
    Some(match part {
        ToolContent::Result => (
            Document::Text(
                message
                    .tool_result
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(&message.content)
                    .into(),
            ),
            language,
        ),
        ToolContent::Input => (
            Document::Json(serde_json::to_value(input).ok()?),
            "json".into(),
        ),
        ToolContent::Field(key) => {
            let value = input?.get(key)?;
            match value {
                serde_json::Value::String(s) => (Document::Text(s.clone()), language),
                _ => (Document::Json(value.clone()), "json".into()),
            }
        }
    })
}
impl WorkbenchController {
    pub(super) fn open_tool_content(
        &mut self,
        id: &str,
        part: &ToolContent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = self.state.as_ref() else {
            return;
        };
        let session = &store.read(cx).model.state.session;
        let Some((document, language)) = document(session, id, part) else {
            return;
        };
        let session_id = session.id.clone();
        let epoch = self.epoch;
        self.operations.retain(|task| !task.is_ready());
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let text = cx
                    .background_executor()
                    .spawn(async move {
                        match document {
                            Document::Text(text) => Ok(text),
                            Document::Json(value) => serde_json::to_string_pretty(&value),
                        }
                    })
                    .await;
                let _ = weak.update_in(cx, |this, window, cx| {
                    if this.epoch != epoch
                        || this
                            .state
                            .as_ref()
                            .is_none_or(|s| s.read(cx).model.state.session.id != session_id)
                    {
                        return;
                    }
                    match text {
                        Ok(text) => {
                            let view =
                                cx.new(|cx| FullToolEditor::new(text, &language, window, cx));
                            window.open_dialog(cx, move |dialog, _, _| {
                                dialog
                                    .title(agentkit_ui::t!("ak.tools.full_content"))
                                    .child(view.clone())
                            });
                        }
                        Err(_) => this.view.update(cx, |v, cx| {
                            v.show_error(agentkit_ui::t!("ak.tools.content_unavailable"), false, cx)
                        }),
                    }
                });
            }));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn complete_contents_come_from_canonical_tools_without_preview_limits() -> anyhow::Result<()> {
        let text = (0..100_000)
            .map(|i| format!("line{i}\n"))
            .collect::<String>();
        let session: g::Session = serde_json::from_value(
            serde_json::json!({"id":"s","workspaceId":"w","workspaceName":"W","lastMessageAt":0,"isProcessing":false,"messages":[{"id":"tool","role":"tool","content":"old","toolResult":text,"timestamp":1,"toolInput":{"path":"src/main.rs","content":"中文\n原始代码"}}]}),
        )?;
        assert!(
            matches!(document(&session,"tool",&ToolContent::Result),Some((Document::Text(s),_)) if s.lines().count()==100_000)
        );
        assert!(
            matches!(document(&session,"tool",&ToolContent::Field("content".into())),Some((Document::Text(s),language)) if s=="中文\n原始代码"&&language=="rust")
        );
        assert!(document(&session, "missing", &ToolContent::Result).is_none());
        assert!(document(&session, "tool", &ToolContent::Field("missing".into())).is_none());
        Ok(())
    }
}
