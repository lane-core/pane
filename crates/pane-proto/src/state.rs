use std::collections::HashMap;
use std::fmt;

use crate::message::{PaneId, PaneKind, PaneRequest};

/// Protocol connection state. Tracks active panes and pending creates.
/// This is local per-connection tracking — not serialized, not sent on the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolState {
    /// Not yet connected.
    Disconnected,
    /// Connected. Tracks active panes and outstanding create requests.
    Active {
        panes: HashMap<PaneId, PaneKind>,
        pending_creates: u32,
    },
}

/// Errors from invalid protocol state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    /// Tried to operate before connecting.
    NotConnected,
    /// Tried to connect when already connected.
    AlreadyConnected,
    /// Pane id not found in the active pane map.
    UnknownPane { id: PaneId },
    /// Operation not valid for this pane kind.
    WrongPaneKind {
        id: PaneId,
        expected: &'static str,
        got: PaneKind,
    },
    /// No pending create to activate.
    NoPendingCreate,
    /// Pane id already exists.
    DuplicatePaneId { id: PaneId },
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConnected => write!(f, "not connected"),
            Self::AlreadyConnected => write!(f, "already connected"),
            Self::UnknownPane { id } => write!(f, "unknown pane id {}", id.get()),
            Self::WrongPaneKind { id, expected, got } => {
                write!(
                    f,
                    "pane {}: expected {} pane, got {:?}",
                    id.get(),
                    expected,
                    got
                )
            }
            Self::NoPendingCreate => write!(f, "no pending create to activate"),
            Self::DuplicatePaneId { id } => write!(f, "pane id {} already exists", id.get()),
        }
    }
}

impl std::error::Error for ProtocolError {}

impl ProtocolState {
    /// Transition to connected state.
    pub fn connect(&self) -> Result<ProtocolState, ProtocolError> {
        match self {
            ProtocolState::Disconnected => Ok(ProtocolState::Active {
                panes: HashMap::new(),
                pending_creates: 0,
            }),
            _ => Err(ProtocolError::AlreadyConnected),
        }
    }

    /// Apply a client request, returning the new state or an error.
    pub fn apply(&self, request: &PaneRequest) -> Result<ProtocolState, ProtocolError> {
        match self {
            ProtocolState::Disconnected => Err(ProtocolError::NotConnected),
            ProtocolState::Active {
                panes,
                pending_creates,
            } => {
                let mut panes = panes.clone();
                let mut pending = *pending_creates;

                match request {
                    PaneRequest::Create { .. } => {
                        pending += 1;
                    }
                    PaneRequest::Close { id } => {
                        if panes.remove(id).is_none() {
                            return Err(ProtocolError::UnknownPane { id: *id });
                        }
                    }
                    PaneRequest::WriteCells { id, .. } | PaneRequest::Scroll { id, .. } => {
                        match panes.get(id) {
                            None => return Err(ProtocolError::UnknownPane { id: *id }),
                            Some(PaneKind::Surface) => {
                                return Err(ProtocolError::WrongPaneKind {
                                    id: *id,
                                    expected: "CellGrid",
                                    got: PaneKind::Surface,
                                });
                            }
                            Some(PaneKind::CellGrid) => {}
                        }
                    }
                    PaneRequest::SetTag { id, .. }
                    | PaneRequest::SetDirty { id, .. }
                    | PaneRequest::RequestGeometry { id, .. } => {
                        if !panes.contains_key(id) {
                            return Err(ProtocolError::UnknownPane { id: *id });
                        }
                    }
                }

                Ok(ProtocolState::Active {
                    panes,
                    pending_creates: pending,
                })
            }
        }
    }

    /// Transition after receiving a Created event from the compositor.
    /// Decrements pending_creates and inserts the pane into the map.
    pub fn activate(
        &self,
        pane_id: PaneId,
        kind: PaneKind,
    ) -> Result<ProtocolState, ProtocolError> {
        match self {
            ProtocolState::Disconnected => Err(ProtocolError::NotConnected),
            ProtocolState::Active {
                panes,
                pending_creates,
            } => {
                if *pending_creates == 0 {
                    return Err(ProtocolError::NoPendingCreate);
                }
                if panes.contains_key(&pane_id) {
                    return Err(ProtocolError::DuplicatePaneId { id: pane_id });
                }
                let mut panes = panes.clone();
                panes.insert(pane_id, kind);
                Ok(ProtocolState::Active {
                    panes,
                    pending_creates: pending_creates - 1,
                })
            }
        }
    }
}
