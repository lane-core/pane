//! Internal unified message type for the looper.
//!
//! The looper receives from two sources:
//! 1. The dispatcher thread (compositor messages)
//! 2. Self-delivery (Messenger::send_message from worker threads)
//!
//! Both funnel into a single mpsc channel as LooperMessage variants.
//!
//! When Tier 2 features add new protocol relationships (clipboard,
//! inter-pane, services), each should be a separate typed channel
//! into the looper with multi-source select — not more variants here.
//! See serena memory `pane/session_type_design_principles` principle C1.

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
}
