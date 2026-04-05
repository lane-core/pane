//! Framework protocols bundled with pane-session.
//!
//! Only Lifecycle lives here — it's the universal protocol that
//! Handler is sugar over. Every pane speaks Lifecycle.
//!
//! Display, Clipboard, Routing, and application-defined protocols
//! are developed in their own crates using pane-session as a
//! dependency. They define their own Protocol + Message types
//! and consumers implement Handles<P> for them.

pub mod lifecycle;
