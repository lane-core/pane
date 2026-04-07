//! Synchronous blocking request for non-looper threads.
//!
//! `send_and_wait` serializes a request, sends it to the looper
//! via a calloop channel for dispatch entry installation, then
//! blocks the calling thread on a oneshot until the reply arrives,
//! the request fails, or the timeout expires.
//!
//! The looper installs the dispatch entry and sends the wire frame
//! atomically from the looper thread, preserving the
//! install-before-wire invariant. The oneshot bridges the reply
//! back to the blocked caller without touching the handler or
//! dispatch table from outside the looper thread.
//!
//! Design heritage: Plan 9 devmnt.c mountio()
//! (reference/plan9/src/sys/src/9/port/devmnt.c:772-826) linked
//! the Mntrpc onto m->queue under spinlock (line 786-790), wrote
//! the request (line 798), then blocked until mountmux dispatched
//! the reply. BeOS BMessenger::SendMessage(BMessage*, BMessage*,
//! bigtime_t) created a temporary reply port, sent the message,
//! and blocked on port_buffer_size_etc with a timeout
//! (src/kits/app/Messenger.cpp:409-472).

use std::sync::mpsc as std_mpsc;

/// Error returned by `ServiceHandle::send_and_wait`.
///
/// Design heritage: Plan 9 mountio() used waserror()/nexterror()
/// for three failure modes: flush (cancel), I/O error, and
/// interrupted. BeOS SendMessage returned status_t with
/// B_TIMED_OUT, B_BAD_PORT_ID (disconnected), B_ERROR.
/// pane uses a typed enum — same failure modes, compile-time
/// exhaustive matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendAndWaitError {
    /// The timeout expired before a reply arrived.
    Timeout,
    /// The looper shut down while the caller was blocked.
    /// The oneshot sender was dropped without sending.
    Disconnected,
    /// The remote handler dropped the ReplyPort without replying
    /// (ServiceFrame::Failed).
    Failed,
    /// The request message could not be serialized.
    SerializationError,
}

impl std::fmt::Display for SendAndWaitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendAndWaitError::Timeout => write!(f, "send_and_wait timed out"),
            SendAndWaitError::Disconnected => write!(f, "looper shut down while blocked"),
            SendAndWaitError::Failed => write!(f, "remote handler dropped ReplyPort"),
            SendAndWaitError::SerializationError => write!(f, "request serialization failed"),
        }
    }
}

impl std::error::Error for SendAndWaitError {}

/// Internal result type sent through the oneshot from the looper
/// to the blocked caller. Contains raw reply bytes (on success)
/// or a failure signal.
pub(crate) type SyncReplyResult = Result<Vec<u8>, SendAndWaitError>;

/// A synchronous request that the looper processes on behalf of
/// a blocked non-looper thread. The looper installs a dispatch
/// entry, sends the wire frame, and routes the reply through
/// `reply_tx`.
///
/// The caller blocks on the receiving end of `reply_tx`.
pub(crate) struct SyncRequest {
    /// Session the request targets.
    pub session_id: u16,
    /// Inner payload bytes (protocol tag + serialized message).
    /// The looper wraps this in `ServiceFrame::Request` with the
    /// allocated token before sending to the wire.
    pub wire_payload: Vec<u8>,
    /// Oneshot sender for delivering the raw reply bytes (or error)
    /// back to the blocked caller.
    pub reply_tx: std_mpsc::SyncSender<SyncReplyResult>,
}
