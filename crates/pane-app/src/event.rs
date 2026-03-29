use pane_proto::event::{KeyEvent, MouseEvent};
use pane_proto::message::PaneId;
use pane_proto::protocol::{CompToClient, PaneGeometry};

use crate::exit::ExitReason;

/// Events delivered to a pane's event handler.
///
/// This is flatter than `CompToClient` — PaneId is stripped (the kit
/// demuxes before the developer sees events), and nested structures
/// are eliminated. The developer matches on this enum directly.
#[derive(Debug, Clone, PartialEq)]
pub enum PaneMessage {
    /// The pane is ready (initial geometry received).
    Ready(PaneGeometry),
    /// The pane was resized.
    Resize(PaneGeometry),
    /// The pane gained focus.
    Focus,
    /// The pane lost focus.
    Blur,
    /// Keyboard input.
    Key(KeyEvent),
    /// Mouse input.
    Mouse(MouseEvent),
    /// The compositor requests this pane to close.
    Close,
    /// The command surface was activated.
    CommandActivated,
    /// The command surface was dismissed.
    CommandDismissed,
    /// A command was executed from the command surface.
    CommandExecuted {
        command: String,
        args: String,
    },
    /// The compositor requests completions for the command surface.
    CompletionRequest {
        token: u64,
        input: String,
    },
    /// The connection to the compositor was lost.
    Disconnected,
    /// A monitored pane exited. Delivered when a pane that this handler
    /// is monitoring (via PaneHandle::monitor()) exits for any reason.
    /// Erlang-style crash propagation through the event loop.
    PaneExited {
        pane: PaneId,
        reason: ExitReason,
    },
}

impl PaneMessage {
    /// Convert a CompToClient message to a PaneEvent for the given pane.
    /// Takes ownership to avoid cloning — the message comes from recv().
    /// Returns None if the message is for a different pane or handled internally.
    pub fn from_comp(msg: CompToClient, pane: PaneId) -> Option<PaneMessage> {
        match msg {
            CompToClient::Resize { pane: p, geometry } if p == pane =>
                Some(PaneMessage::Resize(geometry)),
            CompToClient::Focus { pane: p } if p == pane =>
                Some(PaneMessage::Focus),
            CompToClient::Blur { pane: p } if p == pane =>
                Some(PaneMessage::Blur),
            CompToClient::Key { pane: p, event } if p == pane =>
                Some(PaneMessage::Key(event)),
            CompToClient::Mouse { pane: p, event } if p == pane =>
                Some(PaneMessage::Mouse(event)),
            CompToClient::Close { pane: p } if p == pane =>
                Some(PaneMessage::Close),
            CompToClient::CommandActivated { pane: p } if p == pane =>
                Some(PaneMessage::CommandActivated),
            CompToClient::CommandDismissed { pane: p } if p == pane =>
                Some(PaneMessage::CommandDismissed),
            CompToClient::CommandExecuted { pane: p, command, args } if p == pane =>
                Some(PaneMessage::CommandExecuted { command, args }),
            CompToClient::CompletionRequest { pane: p, token, input } if p == pane =>
                Some(PaneMessage::CompletionRequest { token, input }),
            _ => None,
        }
    }
}
