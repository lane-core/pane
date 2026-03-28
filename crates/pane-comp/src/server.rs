//! Re-export pane-server types for use within the compositor.
//!
//! The protocol server logic lives in the pane-server crate so it
//! can be `cargo check`ed on macOS without smithay dependencies.

pub use pane_server::*;
