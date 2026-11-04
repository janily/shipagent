//! Native GPUI Kit Agent workbench. Views emit intents and never call RPC.
#![deny(missing_docs)]
rust_i18n::i18n!("locales", fallback = "en");
mod chat;
/// Native session composer and safe model catalog.
pub mod composer;
/// Native approval queue, focus and acknowledged conversation notices.
pub mod permissions;
/// Isolated, memory-only Kit component examples; enable with `samples`.
#[cfg(feature = "samples")]
pub mod samples;
/// Native session and workspace navigation.
pub mod sidebar;
/// Native tool summaries, bounded previews and full read-only result editors.
pub mod tools;
/// Virtualized transcript, assistant cards and streaming Markdown documents.
pub mod transcript;
mod workbench;
pub use workbench::{WorkbenchIntent, WorkbenchView};
/// Translate a literal application key with named parameters.
#[macro_export]
macro_rules! t {
    ($key:literal $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::translate($key, &[$(stringify!($name)),*], &[$($value.to_string()),*])
    };
}
/// Runtime implementation of the literal-only translation macro.
#[doc(hidden)]
pub fn translate(key: &str, names: &[&str], values: &[String]) -> String {
    let text = _rust_i18n_translate(&rust_i18n::locale(), key);
    rust_i18n::replace_patterns(&text, names, values)
}
/// Choose the owned UI locale (currently zh-CN or en).
pub fn set_locale(locale: &str) {
    rust_i18n::set_locale(if locale.starts_with("zh") {
        "zh-CN"
    } else {
        "en"
    });
}
/// Viewport-aware, serializable shell layout policy.
pub mod layout;
