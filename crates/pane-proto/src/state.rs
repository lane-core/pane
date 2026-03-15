use std::fmt;

use serde::{Deserialize, Serialize};

use crate::message::{PaneId, PaneRequest};

/// Protocol connection state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolState {
    /// Not yet connected.
    Disconnected,
    /// Connected, no pane created yet.
    Connected,
    /// A pane is active.
    Active { pane_id: PaneId },
}

/// Errors from invalid protocol state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    /// Tried to operate before connecting.
    NotConnected,
    /// Tried to create a pane when one is already active.
    PaneAlreadyActive { existing: PaneId },
    /// Tried to operate on a pane before creating one.
    NoPaneActive,
    /// Operated on wrong pane id.
    WrongPaneId { expected: PaneId, got: PaneId },
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConnected => write!(f, "not connected"),
            Self::PaneAlreadyActive { existing } => {
                write!(f, "pane {} already active, close it first", existing.get())
            }
            Self::NoPaneActive => write!(f, "no pane active, send Create first"),
            Self::WrongPaneId { expected, got } => {
                write!(f, "wrong pane id: expected {}, got {}", expected.get(), got.get())
            }
        }
    }
}

impl std::error::Error for ProtocolError {}

impl ProtocolState {
    /// Apply a request and return the new state, or an error if the
    /// transition is invalid.
    pub fn apply(&self, request: &PaneRequest) -> Result<ProtocolState, ProtocolError> {
        match (self, request) {
            (ProtocolState::Disconnected, _) => Err(ProtocolError::NotConnected),

            (ProtocolState::Connected, PaneRequest::Create { .. }) => {
                // Compositor will assign the id via PaneEvent::Created.
                // For state tracking, we stay Connected until we see the response.
                // But for request validation, Create is valid here.
                Ok(ProtocolState::Connected)
            }
            (ProtocolState::Connected, _) => Err(ProtocolError::NoPaneActive),

            (ProtocolState::Active { pane_id }, PaneRequest::Create { .. }) => {
                Err(ProtocolError::PaneAlreadyActive { existing: *pane_id })
            }
            (ProtocolState::Active { pane_id }, PaneRequest::Close { id }) => {
                if id == pane_id {
                    Ok(ProtocolState::Connected)
                } else {
                    Err(ProtocolError::WrongPaneId { expected: *pane_id, got: *id })
                }
            }
            (ProtocolState::Active { pane_id }, req) => {
                let req_id = match req {
                    PaneRequest::WriteCells { id, .. }
                    | PaneRequest::Scroll { id, .. }
                    | PaneRequest::SetTag { id, .. }
                    | PaneRequest::SetDirty { id, .. }
                    | PaneRequest::RequestGeometry { id, .. } => id,
                    PaneRequest::Create { .. } | PaneRequest::Close { .. } => unreachable!(),
                };
                if req_id == pane_id {
                    Ok(ProtocolState::Active { pane_id: *pane_id })
                } else {
                    Err(ProtocolError::WrongPaneId { expected: *pane_id, got: *req_id })
                }
            }
        }
    }

    /// Transition to Connected state (called after transport is established).
    pub fn connect(&self) -> Result<ProtocolState, ProtocolError> {
        match self {
            ProtocolState::Disconnected => Ok(ProtocolState::Connected),
            _ => Ok(self.clone()),
        }
    }

    /// Transition to Active after receiving a Created event from compositor.
    pub fn activate(&self, pane_id: PaneId) -> Result<ProtocolState, ProtocolError> {
        match self {
            ProtocolState::Connected => Ok(ProtocolState::Active { pane_id }),
            ProtocolState::Active { pane_id: existing } => {
                Err(ProtocolError::PaneAlreadyActive { existing: *existing })
            }
            ProtocolState::Disconnected => Err(ProtocolError::NotConnected),
        }
    }
}
