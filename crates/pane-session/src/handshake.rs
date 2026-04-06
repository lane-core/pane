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
//!
//! Design heritage: Plan 9 Tversion/Rversion negotiated protocol
//! version and max message size (version(5),
//! reference/plan9/man/5/version:19-48). Rerror provided explicit
//! rejection on any T-message (intro(5), 0intro:325-331). BeOS
//! AS_CREATE_APP sent team_id/port/signature
//! (src/kits/app/Application.cpp:1402-1416) and got back a status_t
//! via FlushWithReply (Application.cpp:1423). pane's explicit
//! Result<Welcome, Rejection> combines both: rich rejection reasons
//! (Plan 9 Rerror's explicitness) with typed structure (not Be's
//! bare status_t integer).

use serde::{Serialize, Deserialize};
use pane_proto::ServiceId;

/// The handshake protocol from the client's perspective.
/// Send Hello, receive either Welcome (accepted) or Rejection (declined).
pub type ClientHandshake = par::exchange::Send<Hello, par::exchange::Recv<Result<Welcome, Rejection>>>;

/// The handshake protocol from the server's perspective (dual).
pub type ServerHandshake = par::Dual<ClientHandshake>;

/// A service this pane implements for others.
///
/// Declared in Hello so the server's provider index is populated
/// at handshake time.
///
/// Design heritage: Plan 9's Tattach carried aname (the file tree
/// to mount) — the client declared what it offered in the
/// namespace. BeOS's AS_CREATE_APP carried the app signature,
/// which the roster used to index capabilities. ServiceProvision
/// combines both: a typed service identity with a version, declared
/// upfront for routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceProvision {
    pub service: ServiceId,
    pub version: u32,
}

/// Client → Server: initial connection message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hello {
    pub version: u32,
    pub max_message_size: u32,
    pub interests: Vec<ServiceInterest>,
    /// Services this pane provides for others.
    pub provides: Vec<ServiceProvision>,
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
