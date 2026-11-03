use crate::t;
use gpui_kit::component::input::{Editor, EditorState};
use gpui_kit::{prelude::*, *};
/// Complete in-memory tool content in a virtualized native read-only editor.
pub struct FullToolEditor {
    state: Entity<EditorState>,
}
impl FullToolEditor {
    /// Create a selectable, copyable document; no file is written or executed.
    pub fn new(text: String, language: &str, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| {
            let mut state = EditorState::new(window, cx)
                .language(language.to_owned())
                .line_number(true)
                .default_value(text);
            state.set_readonly(true, cx);
            state
        });
        Self { state }
    }
    /// Inspect the native document in component sample builds.
    #[cfg(feature = "samples")]
    pub fn state(&self) -> &Entity<EditorState> {
        &self.state
    }
}
impl Render for FullToolEditor {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("tool-full-editor")
            .w_full()
            .child(
                Editor::new(&self.state)
                    .readonly(true)
                    .h(px(360.))
                    .aria_label(t!("ak.tools.full_content")),
            )
            .test_support()
    }
}
