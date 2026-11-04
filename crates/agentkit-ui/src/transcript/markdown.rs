use super::*;
use gpui_kit::component::text::{TextView, TextViewState};
/// One selectable Markdown document, updated incrementally for one message ID.
pub struct MarkdownView {
    text: String,
    state: Entity<TextViewState>,
    streaming: bool,
    fade: bool,
}
impl MarkdownView {
    /// Build a Kit Markdown document with syntax highlighting and tables.
    pub fn new(text: &str, streaming: bool, cx: &mut Context<Self>) -> Self {
        Self {
            text: text.into(),
            state: cx.new(|cx| TextViewState::markdown(text, cx)),
            streaming,
            fade: true,
        }
    }
    /// Apply the owning chat view's display preference without a global mutable copy.
    pub fn set_stream_fade(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.fade != enabled {
            self.fade = enabled;
            cx.notify();
        }
    }
    /// Append an authoritative streaming delta without rebuilding the document.
    pub fn append(&mut self, delta: &str, cx: &mut Context<Self>) {
        self.text.push_str(delta);
        self.streaming = true;
        self.state.update(cx, |s, cx| s.push_str(delta, cx));
        cx.notify();
    }
    /// Correct the final document in place, preserving its Entity identity.
    pub fn replace(&mut self, text: &str, streaming: bool, cx: &mut Context<Self>) {
        if self.text != text {
            self.state.update(cx, |s, cx| s.set_text(text, cx));
            self.text = text.into();
        }
        self.streaming = streaming;
        cx.notify();
    }
    /// Underlying selectable document for native component verification.
    #[cfg(feature = "samples")]
    pub fn text_state(&self) -> &Entity<TextViewState> {
        &self.state
    }
    /// Current canonical Markdown source (never a debug representation).
    pub fn text(&self) -> &str {
        &self.text
    }
}
impl EventEmitter<ChatIntent> for MarkdownView {}
impl Render for MarkdownView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.weak_entity();
        div()
            .id("markdown-document")
            .w_full()
            .child(
                TextView::new(&self.state)
                    .selectable(true)
                    .stream_fade(self.streaming && self.fade)
                    .on_link_click(move |url, _, _, cx| {
                        let url = url.to_string();
                        let _ = weak.update(cx, |_, cx| cx.emit(ChatIntent::LinkClicked(url)));
                    }),
            )
            .test_support()
    }
}
