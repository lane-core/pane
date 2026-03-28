//! Internal unified message type for the looper.
//!
//! The looper receives from two sources:
//! 1. The dispatcher thread (compositor messages)
//! 2. Self-delivery (PaneHandle::post_event from worker threads)
//!
//! Both funnel into a single mpsc channel as LooperMessage variants.

use pane_proto::protocol::CompToClient;
use crate::event::PaneEvent;

/// Internal message wrapper for the looper's unified channel.
/// The looper's recv() gets these; it unwraps before dispatch.
#[derive(Debug)]
pub enum LooperMessage {
    /// A message from the compositor, forwarded by the dispatcher.
    FromComp(CompToClient),
    /// A self-delivered event from a worker thread via PaneHandle::post_event().
    Posted(PaneEvent),
}
