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

/// Internal message wrapper for the looper's unified channel.
/// The looper's recv() gets these; it unwraps before dispatch.
#[derive(Debug)]
pub enum LooperMessage {
    /// A message from the compositor, forwarded by the dispatcher.
    FromComp(CompToClient),
    /// A self-delivered event from a worker thread via Messenger::send_message().
    Posted(Message),
    /// Register a periodic timer. The looper fires `event` every `interval`
    /// until the `cancelled` flag is set.
    AddTimer {
        id: u64,
        event: Message,
        interval: Duration,
        cancelled: Arc<AtomicBool>,
    },
    /// Register a one-shot delayed event.
    AddOneShot {
        event: Message,
        fire_at: Instant,
    },
}
