//! Internal unified message type for the looper.
//!
//! The looper receives from two sources:
//! 1. The dispatcher thread (compositor messages)
//! 2. Self-delivery (Messenger::send_message from worker threads)
//!
//! Both funnel into a single mpsc channel as LooperMessage variants.
//! Timer registration and one-shot scheduling also use this channel,
//! keeping timer state local to the looper thread.
//!
//! When Tier 2 features add new protocol relationships (clipboard,
//! inter-pane, services), each should be a separate typed channel
//! into the looper with multi-source select — not more variants here.
//! See serena memory `pane/session_type_design_principles` principle C1.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use pane_proto::protocol::CompToClient;
use crate::event::Message;
use crate::filter::{FilterId, MessageFilter};
use crate::reply::ReplyPort;

/// Internal message wrapper for the looper's unified channel.
/// The looper's recv() gets these; it unwraps before dispatch.
pub enum LooperMessage {
    /// A message from the compositor, forwarded by the dispatcher.
    FromComp(CompToClient),
    /// A self-delivered event from a worker thread via Messenger::send_message().
    Posted(Message),
    /// Register a periodic timer. The looper calls `make_event` every
    /// `interval` to produce the event, until the `cancelled` flag is set.
    /// Using a factory closure avoids cloning — the event is produced
    /// fresh each fire.
    AddTimer {
        id: u64,
        make_event: Box<dyn Fn() -> Message + Send>,
        interval: Duration,
        cancelled: Arc<AtomicBool>,
    },
    /// Register a one-shot delayed event.
    AddOneShot {
        event: Message,
        fire_at: Instant,
    },
    /// A message that expects a reply. The handler receives both
    /// the message and the ReplyPort.
    Request(Message, ReplyPort),
    /// Add a filter to the chain at runtime. Processed between batches.
    AddFilter {
        id: FilterId,
        filter: Box<dyn MessageFilter>,
    },
    /// Remove a filter by ID. No-op if already removed.
    RemoveFilter {
        id: FilterId,
    },
    /// App-level quit negotiation. The looper calls
    /// handler.quit_requested() and sends the result back.
    QuitRequested {
        response_tx: std::sync::mpsc::Sender<bool>,
    },
    /// Imperative close — handler is not consulted.
    /// Sent after all panes have agreed to quit.
    Quit,
}

impl std::fmt::Debug for LooperMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FromComp(msg) => f.debug_tuple("FromComp").field(msg).finish(),
            Self::Posted(msg) => f.debug_tuple("Posted").field(msg).finish(),
            Self::AddTimer { id, interval, .. } =>
                f.debug_struct("AddTimer").field("id", id).field("interval", interval).finish(),
            Self::AddOneShot { fire_at, .. } =>
                f.debug_struct("AddOneShot").field("fire_at", fire_at).finish(),
            Self::Request(msg, reply) =>
                f.debug_tuple("Request").field(msg).field(reply).finish(),
            Self::AddFilter { id, .. } =>
                f.debug_struct("AddFilter").field("id", id).finish(),
            Self::RemoveFilter { id } =>
                f.debug_struct("RemoveFilter").field("id", id).finish(),
            Self::QuitRequested { .. } => f.write_str("QuitRequested"),
            Self::Quit => f.write_str("Quit"),
        }
    }
}
