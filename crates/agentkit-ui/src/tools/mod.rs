//! Native tool presentation. Collapsed cards retain metadata only.
mod card;
mod diff;
mod editor;
mod model;
mod output;
pub use card::ToolCard;
pub use editor::FullToolEditor;
pub use output::{PreparedToolOutput, ToolOutputEvent, ToolOutputView};
/// Canonical payload requested by an explicit full-content action.
#[derive(Clone, Debug)]
pub enum ToolContent {
    /// Complete original tool result.
    Result,
    /// Complete structured input packet.
    Input,
    /// A single structured input field.
    Field(String),
}

pub(crate) use model::grouped;
/// Infer a native syntax language from a file path; unknown extensions stay plain text.
pub fn language_for_path(path: &str) -> &'static str {
    model::language(path)
}
