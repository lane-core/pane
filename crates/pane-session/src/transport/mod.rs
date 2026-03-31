//! Transport layer for session-typed channels.
//!
//! The transport handles the physical delivery of serialized messages.
//! Session types handle the protocol structure; the transport handles the plumbing.

pub mod memory;
pub mod tcp;
pub mod unix;

use crate::error::SessionError;

/// A bidirectional byte transport for session-typed channels.
///
/// Implementations handle serialization framing, connection management,
/// and error translation to `SessionError`.
pub trait Transport: Sized {
    /// Send raw bytes (already serialized by the session layer).
    fn send_raw(&mut self, data: &[u8]) -> Result<(), SessionError>;

    /// Receive raw bytes (to be deserialized by the session layer).
    /// Returns `Err(SessionError::Disconnected)` if the peer is gone.
    fn recv_raw(&mut self) -> Result<Vec<u8>, SessionError>;
}
