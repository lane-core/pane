//! PaneHandle — a lightweight, cloneable handle for sending messages
//! to the compositor from within event handlers.
//!
//! This is the BMessenger pattern: the handle doesn't own the pane,
//! it just knows how to send messages on its behalf. Pass it to
//! spawned threads for async work (network fetches, computation).

use std::sync::mpsc;

use pane_proto::message::PaneId;
use pane_proto::protocol::ClientToComp;
use pane_proto::tag::{PaneTitle, CommandVocabulary, Completion};

use crate::error::{PaneError, Result};

/// A cloneable handle for sending messages to the compositor on
/// behalf of a specific pane. The BMessenger equivalent.
///
/// Use this inside event handlers and spawned threads to update
/// pane state without owning the Pane itself.
#[derive(Clone)]
pub struct PaneHandle {
    pub(crate) id: PaneId,
    pub(crate) sender: mpsc::Sender<ClientToComp>,
}

impl PaneHandle {
    /// Create a PaneHandle from an ID and sender.
    pub fn new(id: PaneId, sender: mpsc::Sender<ClientToComp>) -> Self {
        PaneHandle { id, sender }
    }

    /// The pane's compositor-assigned ID.
    pub fn id(&self) -> PaneId {
        self.id
    }

    /// Update the pane's title.
    pub fn set_title(&self, title: PaneTitle) -> Result<()> {
        self.send(ClientToComp::SetTitle { pane: self.id, title })
    }

    /// Update the pane's command vocabulary.
    pub fn set_vocabulary(&self, vocabulary: CommandVocabulary) -> Result<()> {
        self.send(ClientToComp::SetVocabulary { pane: self.id, vocabulary })
    }

    /// Update the pane's body content.
    pub fn set_content(&self, content: &[u8]) -> Result<()> {
        self.send(ClientToComp::SetContent {
            pane: self.id,
            content: content.to_vec(),
        })
    }

    /// Respond to a completion request.
    pub fn set_completions(&self, token: u64, completions: Vec<Completion>) -> Result<()> {
        self.send(ClientToComp::CompletionResponse {
            pane: self.id,
            token,
            completions,
        })
    }

    fn send(&self, msg: ClientToComp) -> Result<()> {
        self.sender.send(msg).map_err(|_| PaneError::Disconnected)?;
        Ok(())
    }
}

impl std::fmt::Debug for PaneHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaneHandle")
            .field("id", &self.id)
            .finish()
    }
}
