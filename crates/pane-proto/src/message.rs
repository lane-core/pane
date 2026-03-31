use serde::{Deserialize, Serialize};

/// Globally unique pane identifier.
///
/// UUIDv4 — client-proposed at creation time, compositor confirms
/// or rejects. Globally unique by construction, so panes on different
/// instances never collide. This is what makes the unified namespace
/// work: `/pane/<uuid>/` resolves to exactly one pane regardless of
/// which instance hosts it.
///
/// # BeOS
///
/// Be's app_server used client-chosen handler tokens (int32 from a
/// per-process counter). UUIDs extend this pattern to global scope.
/// The compositor retains the right to reject a proposed ID.
///
/// # Plan 9
///
/// 9P uses client-chosen fids — 32-bit handles the client picks to
/// name positions in the server's file tree. UUIDs serve the same
/// role with global uniqueness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaneId(uuid::Uuid);

impl PaneId {
    /// Create a new random PaneId.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// Create a PaneId from an existing UUID.
    pub fn from_uuid(id: uuid::Uuid) -> Self {
        Self(id)
    }

    /// The inner UUID.
    pub fn uuid(self) -> uuid::Uuid {
        self.0
    }
}

impl Default for PaneId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PaneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Active-phase protocol enums (ClientToComp, CompToClient) are in protocol.rs.
// Session-typed handshake types (ClientHandshake, ServerHandshake) are there too.
