use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};

/// Opaque, compositor-assigned pane identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaneId(NonZeroU32);

impl PaneId {
    /// Create a PaneId. Only the compositor should call this.
    pub fn new(id: NonZeroU32) -> Self {
        Self(id)
    }

    pub fn get(self) -> u32 {
        self.0.get()
    }
}

// Protocol message types (PaneRequest, PaneEvent, etc.) will be defined
// as session-typed protocol enums in protocol.rs once pane-session exists.
// See architecture spec §7 and §12 Phase 2.
