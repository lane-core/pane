//! Pane: non-generic connection identity.
//!
//! Plan 9: the bare process after rfork, before namespace
//! customization. PaneBuilder = bind/mount. run_with = exec.

use pane_session::{Flow, Handler};

use crate::builder::PaneBuilder;

/// A pane's tag — title + command vocabulary.
#[derive(Debug, Clone)]
pub struct Tag {
    pub title: String,
    // TODO: commands
}

impl Tag {
    pub fn new(title: &str) -> Self {
        Tag { title: title.to_string() }
    }
}

/// Pane identity. Stub.
pub type Id = u64;

/// A pane — organized state with an interface for views.
/// Non-generic. Connection identity.
#[must_use = "a Pane must be consumed by run, run_with, or setup"]
pub struct Pane {
    pub(crate) id: Id,
    pub(crate) tag: Tag,
    // TODO: connection, looper_tx
}

impl Drop for Pane {
    fn drop(&mut self) {
        // Close the connection. Server detects disconnect.
    }
}

impl Pane {
    /// Enter the typed setup phase for service registration.
    pub fn setup<H: Handler>(self) -> PaneBuilder<H> {
        PaneBuilder::new(self)
    }

    /// Closure form — no services. Lifecycle messages only.
    pub fn run(
        self,
        _f: impl FnMut(&crate::Messenger, pane_session::protocols::lifecycle::LifecycleMessage) -> Flow,
    ) -> ! {
        // TODO: looper with catch_unwind, calloop
        let _ = self;
        std::process::exit(0)
    }

    /// Struct handler — no services needed.
    pub fn run_with<H: Handler>(self, _handler: H) -> ! {
        // TODO: looper with catch_unwind, calloop
        let _ = self;
        std::process::exit(0)
    }
}
