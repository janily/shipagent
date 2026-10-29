//! GPUI Kit phase-zero workbench. Views emit intents and never call RPC.
#![deny(missing_docs)]
rust_i18n::i18n!("locales", fallback = "en");
mod chat;
mod workbench;
pub use workbench::{WorkbenchIntent, WorkbenchView};
pub(crate) fn tr(key: &str) -> String {
    rust_i18n::t!(key).to_string()
}
/// Choose the owned UI locale (currently zh-CN or en).
pub fn set_locale(locale: &str) {
    rust_i18n::set_locale(if locale.starts_with("zh") {
        "zh-CN"
    } else {
        "en"
    });
}
