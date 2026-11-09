use super::*;
use agentkit_protocol::generated as g;
use agentkit_store::core::turns::{Turn, get_assistant_turn_ui_key};
use agentkit_ui::transcript::{ChatIntent, MarkdownView, turn_markdown};
use gpui_kit::component::WindowExt;
impl WorkbenchController {
    pub(super) fn handle_chat(
        &mut self,
        action: ChatIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = self.state.clone() else {
            return;
        };
        match action {
            ChatIntent::LoadPlan(id) => self.load_plan(id, window, cx),
            ChatIntent::AcceptPlan {
                message_id,
                compact,
            } => self.accept_plan(message_id, compact, window, cx),
            ChatIntent::OpenToolContent {
                activity_id,
                content,
            } => self.open_tool_content(&activity_id, &content, window, cx),
            ChatIntent::CopyMessage(ref id) | ChatIntent::ViewTurn(ref id) => {
                let detail = matches!(action, ChatIntent::ViewTurn(_));
                let model = &store.read(cx).model;
                let text = model
                    .state
                    .session
                    .messages
                    .iter()
                    .find(|m| &m.id == id)
                    .map(|m| m.content.clone())
                    .or_else(|| {
                        model.turns.iter().enumerate().find_map(|(row, t)| match t {
                            Turn::Assistant(t) if &get_assistant_turn_ui_key(t, row) == id => {
                                Some(turn_markdown(t))
                            }
                            _ => None,
                        })
                    });
                if let Some(text) = text {
                    if detail {
                        let markdown = cx.new(|cx| MarkdownView::new(&text, false, cx));
                        self.chat_dialog_subscription = Some(cx.subscribe_in(
                            &markdown,
                            window,
                            |this, _, event, window, cx| {
                                this.handle_chat(event.clone(), window, cx)
                            },
                        ));
                        let weak = cx.weak_entity();
                        window.open_dialog(cx, move |dialog, _, _| {
                            let weak = weak.clone();
                            dialog
                                .title(agentkit_ui::t!("ak.chat.details"))
                                .child(markdown.clone())
                                .on_close(move |_, _, cx| {
                                    let _ = weak.update(cx, |this, _| {
                                        this.chat_dialog_subscription = None;
                                    });
                                })
                        });
                    } else {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }
                }
            }
            ChatIntent::ErrorAction { message_id, key } => {
                let Some(action) =
                    error_action(&store.read(cx).model.state.session, &message_id, &key)
                else {
                    return;
                };
                match action {
                    ErrorAction::Retry(user) => {
                        self.handle(WorkbenchIntent::ResendMessage(user), window, cx)
                    }
                    ErrorAction::Settings => self
                        .view
                        .update(cx, |v, cx| v.show_configuration(window, cx)),
                    ErrorAction::Link(url) => self.open_link(&url, window, cx),
                }
            }
            ChatIntent::LinkClicked(url) => self.open_link(&url, window, cx),
            ChatIntent::OpenFile(path) => cx.reveal_path(std::path::Path::new(&path)),
            ChatIntent::BranchFrom(_) => {} // Routed through the normal epoch-guarded load path.
        }
    }
    fn open_link(&mut self, input: &str, _: &mut Window, cx: &mut Context<Self>) {
        let directory = self
            .state
            .as_ref()
            .and_then(|s| s.read(cx).model.state.session.working_directory.clone());
        match link_target(input, directory.as_deref()) {
            Some(LinkTarget::Web(url)) => cx.open_url(&url),
            Some(LinkTarget::File(path)) => cx.reveal_path(&path),
            None => self.view.update(cx, |v, cx| {
                v.show_error(agentkit_ui::t!("ak.chat.link_unavailable"), false, cx)
            }),
        }
    }
}
enum ErrorAction {
    Retry(String),
    Settings,
    Link(String),
}
fn error_action(session: &Session, id: &str, key: &str) -> Option<ErrorAction> {
    let index = session
        .messages
        .iter()
        .position(|m| m.id == id && m.role == g::MessageRole::Error)?;
    let message = &session.messages[index];
    let declared = message
        .error_actions
        .iter()
        .flatten()
        .find(|a| a.key == key);
    let kind = declared.and_then(|a| a.action).or_else(|| {
        (key == "retry" && message.error_can_retry == Some(true))
            .then_some(g::MessageErrorActionsItemAction::Retry)
    })?;
    match kind {
        g::MessageErrorActionsItemAction::Retry if !session.is_processing => session.messages
            [..index]
            .iter()
            .rev()
            .find(|m| m.role == g::MessageRole::User)
            .map(|m| ErrorAction::Retry(m.id.clone())),
        g::MessageErrorActionsItemAction::Settings => Some(ErrorAction::Settings),
        g::MessageErrorActionsItemAction::OpenUrl => {
            declared.and_then(|a| a.url.clone()).map(ErrorAction::Link)
        }
        _ => None,
    }
}
enum LinkTarget {
    Web(String),
    File(PathBuf),
}
fn link_target(input: &str, directory: Option<&str>) -> Option<LinkTarget> {
    let url = url::Url::parse(input).ok().or_else(|| {
        url::Url::from_directory_path(directory?)
            .ok()?
            .join(input)
            .ok()
    })?;
    match url.scheme() {
        "http" | "https" if url.host_str().is_some() => Some(LinkTarget::Web(url.to_string())),
        "file" if url.host_str().is_none() || url.host_str() == Some("localhost") => {
            url.to_file_path().ok().map(LinkTarget::File)
        }
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    #[::core::prelude::v1::test]
    fn actions_are_resolved_from_canonical_errors_and_busy_retry_is_blocked() {
        let mut s:Session=serde_json::from_value(serde_json::json!({"id":"s","workspaceId":"w","workspaceName":"W","messages":[{"id":"u","role":"user","content":"original","timestamp":1},{"id":"e","role":"error","content":"failure","timestamp":2,"errorCanRetry":true,"errorActions":[{"key":"configure","label":"Settings","action":"settings"}]}],"lastMessageAt":2,"isProcessing":false})).unwrap();
        assert!(matches!(error_action(&s,"e","retry"),Some(ErrorAction::Retry(id)) if id=="u"));
        assert!(matches!(
            error_action(&s, "e", "configure"),
            Some(ErrorAction::Settings)
        ));
        assert!(error_action(&s, "e", "injected").is_none());
        assert!(error_action(&s, "u", "retry").is_none());
        s.is_processing = true;
        assert!(error_action(&s, "e", "retry").is_none());
    }
    #[::core::prelude::v1::test]
    fn links_allow_web_and_local_files_but_never_executable_or_remote_file_schemes() {
        assert!(matches!(
            link_target("https://example.com/doc", None),
            Some(LinkTarget::Web(_))
        ));
        for input in [
            "javascript:alert(1)",
            "data:text/html,boom",
            "file://remote/share",
            "shipagent://execute/command",
        ] {
            assert!(link_target(input, None).is_none());
        }
        #[cfg(unix)]
        assert!(
            matches!(link_target("README.md",Some("/tmp/project")),Some(LinkTarget::File(p)) if p==std::path::Path::new("/tmp/project/README.md"))
        );
    }
}
