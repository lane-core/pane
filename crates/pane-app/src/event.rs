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
pub enum Message {
    /// The pane is ready (initial geometry received).
    Ready(PaneGeometry),
    /// The pane was resized.
    Resize(PaneGeometry),
    /// The pane was activated (gained focus).
    Activated,
    /// The pane was deactivated (lost focus).
    Deactivated,
    /// Keyboard input.
    Key(KeyEvent),
    /// Mouse input.
    Mouse(MouseEvent),
    /// The compositor requests this pane to close.
    CloseRequested,
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
    /// Periodic pulse. Delivered at the rate set by
    /// Messenger::set_pulse_rate(). The blessed way to do
    /// animations, status polling, clock ticks.
    Pulse,
    /// The connection to the compositor was lost.
    Disconnected,
    /// A monitored pane exited. Delivered when a pane that this handler
    /// is monitoring (via Messenger::monitor()) exits for any reason.
    /// Erlang-style crash propagation through the event loop.
    PaneExited {
        pane: PaneId,
        reason: ExitReason,
    },
}

impl Message {
    /// Convert a CompToClient message to a Message for the given pane.
    /// Takes ownership to avoid cloning — the message comes from recv().
    /// Returns None if the message is for a different pane or handled internally.
    pub fn from_comp(msg: CompToClient, pane: PaneId) -> Option<Message> {
        match msg {
            CompToClient::Resize { pane: p, geometry } if p == pane =>
                Some(Message::Resize(geometry)),
            CompToClient::Focus { pane: p } if p == pane =>
                Some(Message::Activated),
            CompToClient::Blur { pane: p } if p == pane =>
                Some(Message::Deactivated),
            CompToClient::Key { pane: p, event } if p == pane =>
                Some(Message::Key(event)),
            CompToClient::Mouse { pane: p, event } if p == pane =>
                Some(Message::Mouse(event)),
            CompToClient::Close { pane: p } if p == pane =>
                Some(Message::CloseRequested),
            CompToClient::CommandActivated { pane: p } if p == pane =>
                Some(Message::CommandActivated),
            CompToClient::CommandDismissed { pane: p } if p == pane =>
                Some(Message::CommandDismissed),
            CompToClient::CommandExecuted { pane: p, command, args } if p == pane =>
                Some(Message::CommandExecuted { command, args }),
            CompToClient::CompletionRequest { pane: p, token, input } if p == pane =>
                Some(Message::CompletionRequest { token, input }),
            _ => None,
        }
    }
}
