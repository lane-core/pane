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
    CreatePane {
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
}

/// Server hello — compositor's response to ClientHello.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerHello {
    /// Compositor identifier.
    pub compositor: String,
    /// Protocol version (may differ from client's).
    pub version: u32,
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
}

/// Handshake rejected — compositor rejects the client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rejected {
    /// Reason for rejection.
    pub reason: String,
}

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
