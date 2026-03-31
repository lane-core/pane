//! Active-phase protocol types for compositor ↔ client communication.
//!
//! These are the typed message enums for the bidirectional active phase
//! (Phase 2 of the three-phase protocol model). Session types govern
//! the handshake; these enums govern the active phase where both sides
//! send freely.

use serde::{Deserialize, Serialize};

use crate::event::{KeyEvent, MouseEvent};
use crate::message::PaneId;
use crate::tag::{PaneTitle, CommandVocabulary, Completion};

// --- Active-phase messages ---

/// Messages from a pane-native client to the compositor.
/// Sent during the active phase after handshake completes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClientToComp {
    /// Create a new pane with the given tag configuration.
    /// The client proposes a UUID PaneId; the compositor confirms
    /// (PaneCreated) or rejects (PaneRefused).
    CreatePane {
        pane: PaneId,
        tag: Option<CreatePaneTag>,
    },
    /// Request to close a pane.
    RequestClose {
        pane: PaneId,
    },
    /// Update the pane's title.
    SetTitle {
        pane: PaneId,
        title: PaneTitle,
    },
    /// Update the pane's command vocabulary.
    SetVocabulary {
        pane: PaneId,
        vocabulary: CommandVocabulary,
    },
    /// Update the pane's body content (opaque bytes — format depends
    /// on the content model negotiated during handshake).
    SetContent {
        pane: PaneId,
        content: Vec<u8>,
    },
    /// Respond to a completion request from the compositor.
    CompletionResponse {
        pane: PaneId,
        token: u64,
        completions: Vec<Completion>,
    },
    /// Request a resize. The compositor decides whether to honor it.
    RequestResize {
        pane: PaneId,
        width: u32,
        height: u32,
    },
    /// Declare size limits. The compositor uses these during layout.
    SetSizeLimits {
        pane: PaneId,
        min_width: u32,
        min_height: u32,
        max_width: u32,
        max_height: u32,
    },
    /// Request the pane be hidden or shown.
    SetHidden {
        pane: PaneId,
        hidden: bool,
    },
}

/// Messages from the compositor to a pane-native client.
/// Each variant carries the PaneId for demultiplexing — the kit
/// strips this before presenting events to the developer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CompToClient {
    /// A pane was created. Response to CreatePane.
    PaneCreated {
        pane: PaneId,
        geometry: PaneGeometry,
    },
    /// Pane was resized.
    Resize {
        pane: PaneId,
        geometry: PaneGeometry,
    },
    /// Pane gained focus.
    Focus {
        pane: PaneId,
    },
    /// Pane lost focus.
    Blur {
        pane: PaneId,
    },
    /// Keyboard input.
    Key {
        pane: PaneId,
        event: KeyEvent,
    },
    /// Mouse input.
    Mouse {
        pane: PaneId,
        event: MouseEvent,
    },
    /// Compositor requests the pane to close.
    Close {
        pane: PaneId,
    },
    /// Close acknowledged — the pane has been removed from the layout.
    CloseAck {
        pane: PaneId,
    },
    /// The command surface was activated (user hit the activation key).
    CommandActivated {
        pane: PaneId,
    },
    /// The command surface was dismissed (Escape or focus loss).
    CommandDismissed {
        pane: PaneId,
    },
    /// A command was executed from the command surface.
    CommandExecuted {
        pane: PaneId,
        command: String,
        args: String,
    },
    /// The compositor requests completions for the current input.
    CompletionRequest {
        pane: PaneId,
        token: u64,
        input: String,
    },
}

impl CompToClient {
    /// Extract the PaneId from any variant.
    /// Every CompToClient message carries a PaneId — this is guaranteed
    /// by the enum's structure, not by convention.
    pub fn pane_id(&self) -> PaneId {
        match self {
            CompToClient::PaneCreated { pane, .. } => *pane,
            CompToClient::Resize { pane, .. } => *pane,
            CompToClient::Focus { pane } => *pane,
            CompToClient::Blur { pane } => *pane,
            CompToClient::Key { pane, .. } => *pane,
            CompToClient::Mouse { pane, .. } => *pane,
            CompToClient::Close { pane } => *pane,
            CompToClient::CloseAck { pane } => *pane,
            CompToClient::CommandActivated { pane } => *pane,
            CompToClient::CommandDismissed { pane } => *pane,
            CompToClient::CommandExecuted { pane, .. } => *pane,
            CompToClient::CompletionRequest { pane, .. } => *pane,
        }
    }
}

// --- Handshake types ---

/// Client hello — sent as the first message in the session-typed handshake.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientHello {
    /// Application signature (e.g., "com.example.hello").
    pub signature: String,
    /// Protocol version.
    pub version: u32,
    /// Peer identity for remote connections. `None` for local unix
    /// sockets (where `SO_PEERCRED` provides identity implicitly).
    ///
    /// Sent early (before capability negotiation) so the server can
    /// make access control decisions before proceeding. Mirrors 9P's
    /// Tauth placement before Tattach.
    pub identity: Option<PeerIdentity>,
}

/// Identity of a remote peer, declared during handshake.
///
/// For remote TCP/TLS connections, the server validates this against
/// the TLS client certificate. For local unix sockets, identity is
/// implicit via `SO_PEERCRED` and this field is `None`.
///
/// # BeOS
///
/// No equivalent — Be's app_server connected over kernel ports where
/// identity was ambient. This extends the model for network transparency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerIdentity {
    /// Unix username on the peer's system.
    pub username: String,
    /// Numeric UID on the peer's system.
    pub uid: u32,
    /// Hostname of the peer's system.
    pub hostname: String,
}

/// Server hello — compositor's response to ClientHello.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerHello {
    /// Compositor identifier.
    pub compositor: String,
    /// Protocol version (may differ from client's).
    pub version: u32,
    /// Instance identifier for federation and discovery.
    /// A UUID that uniquely identifies this pane server instance.
    /// Used by pane-roster for cross-instance service discovery
    /// and by pane-fs for routing writes to the owning instance.
    pub instance_id: String,
}

/// Client capabilities — sent after ServerHello.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientCaps {
    /// Requested capabilities.
    pub caps: Vec<String>,
}

/// Handshake accepted — compositor accepts the client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accepted {
    /// Resolved capabilities (intersection of requested and supported).
    pub caps: Vec<String>,
    /// Connection topology, classified by the server from the transport.
    /// The client does not declare this — the server infers it.
    pub topology: ConnectionTopology,
}

/// Connection topology as classified by the server.
///
/// The server knows the transport type (unix socket vs TCP) and
/// classifies the connection accordingly. The client receives this
/// in `Accepted` and uses it for routing decisions (e.g., preferring
/// local handlers over remote ones for latency-sensitive operations).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionTopology {
    /// Local unix socket connection (same machine).
    Local,
    /// Remote network connection (TCP/TLS).
    Remote,
    /// Connection via an intermediate pane instance (future federation).
    Federated,
}

/// Handshake rejected — compositor rejects the client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rejected {
    /// Reason for rejection.
    pub reason: String,
}

// --- Session-typed handshake protocol ---
//
// The three-phase protocol model:
//   Phase 1: Session-typed handshake (these type aliases)
//   Phase 2: Typed enum active phase (ClientToComp / CompToClient)
//   Phase 3: Session-typed teardown (future)
//
// The handshake uses session types to enforce the correct message
// ordering at compile time. After the handshake completes, `finish()`
// reclaims the transport for the active phase.

use pane_session::types::{Send, Recv, Branch, End};

/// The client's view of the handshake protocol.
///
/// ```text
/// Client                    Server
///   ── ClientHello ────────→
///   ←──────────── ServerHello ──
///   ── ClientCaps ─────────→
///   ←── Branch ────────────→
///       ├─ Accepted (end)
///       └─ Rejected (end)
/// ```
///
/// # BeOS
///
/// No BeOS equivalent. BeOS applications connected to app_server
/// and immediately began sending — there was no handshake, no
/// capability negotiation, and no typed protocol phase. The
/// session-typed handshake ensures both sides agree on the protocol
/// version before entering the active phase.
pub type ClientHandshake = Send<ClientHello,
    Recv<ServerHello,
        Send<ClientCaps,
            Branch<
                Recv<Accepted, End>,
                Recv<Rejected, End>,
            >>>>;

/// The server's view of the handshake (dual of ClientHandshake).
/// Automatically derived: Send↔Recv, Select↔Branch.
pub type ServerHandshake = <ClientHandshake as pane_session::dual::HasDual>::Dual;

// --- Supporting types ---

/// Pane geometry as reported by the compositor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneGeometry {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Column count (for text-mode content).
    pub cols: u16,
    /// Row count (for text-mode content).
    pub rows: u16,
}

/// Tag configuration for pane creation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatePaneTag {
    /// Initial title.
    pub title: PaneTitle,
    /// Initial command vocabulary.
    pub vocabulary: CommandVocabulary,
}
