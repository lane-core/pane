//! Scripting protocol — structured, discoverable access to pane state.
//!
//! Session types + optics = the recovery of BeOS's ResolveSpecifier.
//! Every pane is automatable through the same protocol it uses for
//! everything else.
//!
//! TODO(phase-6): implement optic-addressed property access,
//! GetSupportedSuites equivalent, dynamic specifier chain resolution.

/// A property declaration — what optics a pane exposes for scripting.
///
/// The scripting protocol's equivalent of BHandler::GetSupportedSuites().
/// Each property is a named, typed access path into the pane's state.
///
/// TODO(phase-6): implement with actual optic types.
#[derive(Debug, Clone)]
pub struct PropertyDecl {
    /// Property name (e.g., "title", "content", "selection").
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Whether the property is read-only or read-write.
    pub writable: bool,
}

/// A scripting query received from the compositor.
///
/// TODO(phase-6): implement with actual specifier chain types.
#[derive(Debug, Clone)]
pub struct ScriptQuery {
    /// The property being accessed.
    pub property: String,
    /// The operation (get, set, count, etc.).
    pub operation: ScriptOp,
}

/// Scripting operations.
#[derive(Debug, Clone)]
pub enum ScriptOp {
    /// Get a property's value.
    Get,
    /// Set a property's value.
    Set(Vec<u8>),
    /// Count items in a collection property.
    Count,
    /// List available properties (GetSupportedSuites).
    ListProperties,
}

/// A reply token for responding to scripting queries.
///
/// TODO(phase-6): implement with actual response channel.
#[derive(Debug)]
pub struct ScriptReplyToken {
    _token: u64,
}

impl ScriptReplyToken {
    /// Reply with a value.
    pub fn reply(self, _value: &[u8]) -> crate::Result<()> {
        // TODO(phase-6): send response through the protocol
        Ok(())
    }

    /// Reply with an error.
    pub fn reply_error(self, _message: &str) -> crate::Result<()> {
        // TODO(phase-6): send error response
        Ok(())
    }
}
