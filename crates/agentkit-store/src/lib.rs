//! Pure phase-zero state reduction; GPUI views own entities wrapping this state.
#![deny(missing_docs)]
pub mod core;
pub use core::{ChatState, RunState};

/// Optional native entity wrappers; the default store remains GPUI-independent.
#[cfg(feature = "entities")]
pub mod entities;
