//! Handshake types for the pane wire protocol.
//!
//! The handshake is a par session-typed exchange:
//!   Client → Server: Hello
//!   Server → Client: Result<Welcome, Rejection>
//!
//! The server responds with Ok(Welcome) on success or
//! Err(Rejection) on failure. This is a value-level Result
//! inside a single Send/Recv exchange — both branches terminate
//! the par session. Not par's `choose` mechanism.
//!
//! Protocol types defined with par. Executed over a Transport
//! via the bridge module.

use serde::{Serialize, Deserialize};
use pane_proto::ServiceId;

/// The handshake protocol from the client's perspective.
/// Send Hello, receive either Welcome (accepted) or Rejection (declined).
pub type ClientHandshake = par::exchange::Send<Hello, par::exchange::Recv<Result<Welcome, Rejection>>>;

/// The handshake protocol from the server's perspective (dual).
pub type ServerHandshake = par::Dual<ClientHandshake>;

/// Client → Server: initial connection message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hello {
    pub version: u32,
    pub max_message_size: u32,
    pub interests: Vec<ServiceInterest>,
}

/// Server → Client: handshake response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Welcome {
    pub version: u32,
    pub instance_id: String,
    pub max_message_size: u32,
    pub bindings: Vec<ServiceBinding>,
}

/// A service the client wants to use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInterest {
    pub service: ServiceId,
    pub expected_version: u32,
}

/// A service binding from the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceBinding {
    pub service: ServiceId,
    pub session_id: u8,
    pub version: u32,
}

/// Handshake rejection — server explicitly declines the connection.
///
/// Sent as Err(Rejection) in the handshake Result. The client
/// receives this via recv1() and can inspect the reason and
/// optional human-readable message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rejection {
    pub reason: RejectReason,
    pub message: Option<String>,
}

/// Why the server rejected the handshake.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RejectReason {
    VersionMismatch,
    Unauthorized,
    ServerFull,
    ServiceUnavailable,
}
