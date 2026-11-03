use super::diff::prepare_diff;
#[path = "output_render.rs"]
mod render;
use super::{ToolContent, model::*};
use crate::{t, transcript::ChatIntent};
use agentkit_store::core::turns::Activity;
use gpui_kit::component::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    diff::DiffState,
    input::{Editor, EditorState},
};
use gpui_kit::{prelude::*, *};
/// Bounded, comparable tool-preview data without native widget allocations.
pub struct PreparedToolOutput(Preview);
/// Layout updates emitted after asynchronous diff preparation or input expansion.
pub enum ToolOutputEvent {
    /// The owning virtual row should be measured again.
    LayoutChanged,
}
/// A bounded lazy tool-input/result preview. Full content stays in the canonical store.
pub struct ToolOutputView {
    preview: Preview,
    diff: Option<Entity<DiffState>>,
    editor: Option<Entity<EditorState>>,
    loading: bool,
    task: Option<Task<()>>,
}
impl EventEmitter<ChatIntent> for ToolOutputView {}
impl EventEmitter<ToolOutputEvent> for ToolOutputView {}
impl ToolOutputView {
    /// Construct only after the user expands a tool; diff work runs in the background.
    pub fn new(activity: &Activity, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::from_prepared(Self::prepare(activity), window, cx)
    }
    /// Build only bounded data while borrowing the canonical activity.
    pub fn prepare(activity: &Activity) -> PreparedToolOutput {
        PreparedToolOutput(Preview::new(activity))
    }
    /// Whether a prepared payload would leave this native document unchanged.
    pub fn matches(&self, prepared: &PreparedToolOutput) -> bool {
        self.preview == prepared.0
    }
    /// Materialize a previously prepared bounded preview on the native UI thread.
    pub fn from_prepared(
        prepared: PreparedToolOutput,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let preview = prepared.0;

        let mut view = Self {
            preview,
            diff: None,
            editor: None,
            loading: false,
            task: None,
        };
        match &view.preview.source {
            Source::Code | Source::Text => {
                let text = view.preview.result.text.clone();
                let lang = view.preview.language.clone();
                view.editor = Some(cx.new(|cx| {
                    let mut s = EditorState::new(window, cx)
                        .language(lang)
                        .line_number(true)
                        .default_value(text);
                    s.set_readonly(true, cx);
                    s
                }));
            }
            Source::Patch { .. } | Source::Change { .. } => {
                let text = view.preview.result.text.clone();
                view.editor = Some(cx.new(|cx| {
                    let mut s = EditorState::new(window, cx)
                        .language("plaintext")
                        .default_value(text);
                    s.set_readonly(true, cx);
                    s
                }));
                view.loading = true;
                let source = view.preview.source.clone();
                view.task = Some(cx.spawn_in(window, async move |weak, cx| {
                    let files = cx
                        .background_executor()
                        .spawn(async move { prepare_diff(source) })
                        .await;
                    let _ = weak.update_in(cx, |this, window, cx| {
                        this.diff = files.map(|files| cx.new(|cx| DiffState::new(files, cx)));
                        if this.diff.is_none() {
                            let text = this.preview.result.text.clone();
                            this.editor = Some(cx.new(|cx| {
                                let mut s = EditorState::new(window, cx)
                                    .language("plaintext")
                                    .default_value(text);
                                s.set_readonly(true, cx);
                                s
                            }));
                        }
                        this.loading = false;
                        cx.emit(ToolOutputEvent::LayoutChanged);
                        cx.notify();
                    });
                }));
            }
        }
        view
    }
    fn request(&self, content: ToolContent, cx: &mut Context<Self>) {
        cx.emit(ChatIntent::OpenToolContent {
            activity_id: self.preview.id.clone(),
            content,
        });
    }
    /// Retained preview text for sample verification of line and byte limits.
    #[cfg(feature = "samples")]
    pub fn preview_text(&self) -> &str {
        &self.preview.result.text
    }
    /// Access a parsed native diff after the background task completes.
    #[cfg(feature = "samples")]
    pub fn diff_state(&self) -> Option<&Entity<DiffState>> {
        self.diff.as_ref()
    }
}
