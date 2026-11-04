//! Five native reverse-RPC capabilities. Browser automation is never advertised.
use agentkit_protocol::{ErrorCode, WireError};
use gpui_kit::{AnyWindowHandle, App, Task};
use std::{cell::RefCell, rc::Rc};
mod bridge;
mod confirm;
mod confirm_view;
mod fallback;
mod files;
mod native;
mod params;
mod transient;
pub use params::{Action, ConfirmSpec, FileFilter, FileSpec, parse};
/// Names installed before the WebSocket handshake.
pub const NAMES: [&str; 5] = [
    "client:openExternal",
    "client:openPath",
    "client:showItemInFolder",
    "client:confirmDialog",
    "client:openFileDialog",
];
fn failure(message: &str) -> WireError {
    WireError {
        code: ErrorCode::HandlerError,
        message: message.into(),
        data: None,
    }
}
fn unavailable(message: &str) -> WireError {
    WireError {
        code: ErrorCode::CapabilityUnavailable,
        message: message.into(),
        data: None,
    }
}
type Recent = Rc<RefCell<Option<AnyWindowHandle>>>;
/// Application-owned foreground dispatcher; keep a clone for the window lifetime.
#[derive(Clone)]
pub struct NativeCapabilities {
    handlers: std::collections::BTreeMap<String, agentkit_client::CapabilityHandler>,
    recent: Recent,
    _task: Rc<Task<()>>,
}
impl NativeCapabilities {
    /// Start a bounded, cancelable dispatcher on the GPUI foreground executor.
    pub fn install(cx: &mut App) -> Self {
        let (handlers, requests) = bridge::channel();
        let recent: Recent = Rc::new(RefCell::new(None));
        let selected = recent.clone();
        let modal = std::sync::Arc::new(smol::lock::Mutex::new(()));
        let task = cx.spawn(async move |cx| {
            let mut tasks: Vec<Task<()>> = Vec::new();
            while let Ok(request) = requests.recv().await {
                tasks.retain(|t| !t.is_ready());
                if !request.live() {
                    continue;
                }
                let recent = selected.clone();
                let modal = modal.clone();
                tasks.push(
                    cx.spawn(async move |cx| native::execute(request, recent, modal, cx).await),
                );
            }
        });
        Self {
            handlers,
            recent,
            _task: Rc::new(task),
        }
    }
    /// Clone the registered handlers for initial connection options.
    pub fn handlers(
        &self,
    ) -> std::collections::BTreeMap<String, agentkit_client::CapabilityHandler> {
        self.handlers.clone()
    }
    /// Track activation so inactive-app dialogs still target the most recently focused window.
    pub fn remember(&self, window: AnyWindowHandle) {
        *self.recent.borrow_mut() = Some(window);
    }
}
fn parent(recent: &Recent, cx: &App) -> Option<AnyWindowHandle> {
    let windows = cx.windows();
    cx.active_window()
        .or_else(|| {
            recent
                .borrow()
                .filter(|w| windows.iter().any(|x| x.window_id() == w.window_id()))
        })
        .or_else(|| windows.last().copied())
}

#[cfg(test)]
mod tests;
