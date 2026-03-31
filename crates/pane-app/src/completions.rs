//! Completion reply handle — typed ownership for completion responses.
//!
//! Sibling pattern to [`ReplyPort`](crate::ReplyPort): consumed by
//! `.reply()`, Drop sends an empty completion list. Same ownership
//! discipline, different transport destination (compositor wire via
//! `Sender<ClientToComp>`, not looper channel).

use std::sync::mpsc;

use pane_proto::message::PaneId;
use pane_proto::protocol::ClientToComp;
use pane_proto::tag::Completion;

/// Reply handle for completion requests from the compositor.
///
/// Created internally when a `CompToClient::CompletionRequest` arrives.
/// The handler calls [`reply`](CompletionReplyPort::reply) with the
/// completions; if dropped without replying, an empty completion list
/// is sent automatically (the command surface shows "no completions"
/// rather than hanging).
///
/// # BeOS
///
/// No direct equivalent. Be's completion was synchronous within
/// app_server. Pane's completion is async via the compositor protocol.
#[must_use = "dropping without replying sends empty completions to the compositor"]
pub struct CompletionReplyPort {
    pane: PaneId,
    token: u64,
    sender: Option<mpsc::Sender<ClientToComp>>,
}

impl CompletionReplyPort {
    /// Create a new completion reply port.
    #[doc(hidden)] // Public for integration tests, not part of the developer API.
    pub fn new(
        pane: PaneId,
        token: u64,
        sender: mpsc::Sender<ClientToComp>,
    ) -> Self {
        Self {
            pane,
            token,
            sender: Some(sender),
        }
    }

    /// Reply with completions, consuming the handle.
    pub fn reply(mut self, completions: Vec<Completion>) {
        self.send(completions);
    }

    /// The completion request token (for logging/debugging only —
    /// the handler should not need to manage this).
    pub fn token(&self) -> u64 {
        self.token
    }

    fn send(&mut self, completions: Vec<Completion>) {
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(ClientToComp::CompletionResponse {
                pane: self.pane,
                token: self.token,
                completions,
            });
        }
    }
}

impl Drop for CompletionReplyPort {
    fn drop(&mut self) {
        // If sender is still Some, reply() was never called.
        // Send empty completions so the compositor doesn't hang.
        self.send(vec![]);
    }
}

impl std::fmt::Debug for CompletionReplyPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompletionReplyPort")
            .field("pane", &self.pane)
            .field("token", &self.token)
            .field("replied", &self.sender.is_none())
            .finish()
    }
}
