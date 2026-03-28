use pane_proto::event::{KeyEvent, MouseEvent};
use pane_proto::message::PaneId;
use pane_proto::protocol::{CompToClient, PaneGeometry};

/// Events delivered to a pane's event handler.
///
/// This is flatter than `CompToClient` — PaneId is stripped (the kit
/// demuxes before the developer sees events), and nested structures
/// are eliminated. The developer matches on this enum directly.
#[derive(Debug, Clone, PartialEq)]
pub enum PaneEvent {
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
}

impl PaneEvent {
    /// Convert a CompToClient message to a PaneEvent for the given pane.
    /// Takes ownership to avoid cloning — the message comes from recv().
    /// Returns None if the message is for a different pane or handled internally.
    pub fn from_comp(msg: CompToClient, pane: PaneId) -> Option<PaneEvent> {
        match msg {
            CompToClient::Resize { pane: p, geometry } if p == pane =>
                Some(PaneEvent::Resize(geometry)),
            CompToClient::Focus { pane: p } if p == pane =>
                Some(PaneEvent::Focus),
            CompToClient::Blur { pane: p } if p == pane =>
                Some(PaneEvent::Blur),
            CompToClient::Key { pane: p, event } if p == pane =>
                Some(PaneEvent::Key(event)),
            CompToClient::Mouse { pane: p, event } if p == pane =>
                Some(PaneEvent::Mouse(event)),
            CompToClient::Close { pane: p } if p == pane =>
                Some(PaneEvent::Close),
            CompToClient::CommandActivated { pane: p } if p == pane =>
                Some(PaneEvent::CommandActivated),
            CompToClient::CommandDismissed { pane: p } if p == pane =>
                Some(PaneEvent::CommandDismissed),
            CompToClient::CommandExecuted { pane: p, command, args } if p == pane =>
                Some(PaneEvent::CommandExecuted { command, args }),
            CompToClient::CompletionRequest { pane: p, token, input } if p == pane =>
                Some(PaneEvent::CompletionRequest { token, input }),
            _ => None,
        }
    }
}
