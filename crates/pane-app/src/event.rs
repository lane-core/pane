use std::any::Any;

use pane_proto::event::{KeyEvent, MouseEvent};
use pane_proto::message::PaneId;
use pane_proto::protocol::{CompToClient, PaneGeometry};

use crate::exit::ExitReason;

/// Events delivered to a pane's event handler.
///
/// The typed replacement for BeOS's `BMessage` `what` dispatch.
/// Where Be used runtime matching on a `uint32` with untyped data
/// fields, pane uses exhaustive enum matching with typed payloads.
/// The compiler ensures every event variant is handled or explicitly
/// ignored.
///
/// For application-defined events (worker thread results, async
/// completions), use [`App`](Message::App) via
/// [`Messenger::post_app_message`](crate::Messenger::post_app_message).
/// These are delivered to [`Handler::message_received`](crate::Handler::message_received).
///
/// PaneId is stripped (the kit demuxes before you see events), and
/// nested structures are eliminated. When using [`Pane::run`](crate::Pane::run)
/// (closure form), you match on Message directly. When using
/// [`Pane::run_with`](crate::Pane::run_with) (Handler form), each variant
/// dispatches to the corresponding [`Handler`](crate::Handler) method.
#[derive(Debug)]
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
    /// Application-defined message posted from a worker thread.
    ///
    /// Use [`Messenger::post_app_message`] to send these. The looper
    /// delivers them to [`Handler::message_received`].
    ///
    /// # BeOS
    ///
    /// Equivalent to app-defined `BMessage` `what` codes dispatched
    /// through `BHandler::MessageReceived`.
    App(Box<dyn Any + Send>),
    /// Reply to a request sent via [`Messenger::send_and_wait`] or
    /// the async request API. Dispatches to [`Handler::reply_received`].
    ///
    /// # BeOS
    ///
    /// Equivalent to the reply `BMessage` delivered via `BMessage::SendReply`.
    Reply {
        token: u64,
        payload: Box<dyn Any + Send>,
    },
    /// A request's reply failed — the target dropped the [`ReplyPort`](crate::ReplyPort)
    /// without replying, or the target pane exited mid-conversation.
    /// Dispatches to [`Handler::reply_failed`].
    ///
    /// Per EAct principle C3: per-conversation failure notification.
    ReplyFailed {
        token: u64,
    },
}

impl Clone for Message {
    fn clone(&self) -> Self {
        match self {
            Self::Ready(g) => Self::Ready(*g),
            Self::Resize(g) => Self::Resize(*g),
            Self::Activated => Self::Activated,
            Self::Deactivated => Self::Deactivated,
            Self::Key(e) => Self::Key(e.clone()),
            Self::Mouse(e) => Self::Mouse(e.clone()),
            Self::CloseRequested => Self::CloseRequested,
            Self::CommandActivated => Self::CommandActivated,
            Self::CommandDismissed => Self::CommandDismissed,
            Self::CommandExecuted { command, args } =>
                Self::CommandExecuted { command: command.clone(), args: args.clone() },
            Self::CompletionRequest { token, input } =>
                Self::CompletionRequest { token: *token, input: input.clone() },
            Self::Pulse => Self::Pulse,
            Self::Disconnected => Self::Disconnected,
            Self::PaneExited { pane, reason } =>
                Self::PaneExited { pane: *pane, reason: reason.clone() },
            Self::App(_) =>
                panic!("App messages are consumed, not cloned"),
            Self::Reply { .. } =>
                panic!("Reply messages are consumed, not cloned"),
            Self::ReplyFailed { token } =>
                Self::ReplyFailed { token: *token },
        }
    }
}

impl Message {
    /// Convert a CompToClient message to a Message for the given pane.
    /// Takes ownership to avoid cloning — the message comes from recv().
    /// Returns None if the message is for a different pane or handled internally.
    pub fn try_from_comp(msg: CompToClient, pane: PaneId) -> Option<Message> {
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
