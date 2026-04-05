//! ExitReason: looper-internal exit disposition.
//!
//! NOT in the handler API. The handler returns Flow; the looper
//! translates the outcome into ExitReason for the PaneExited
//! broadcast. Stripped of error details — failure reason is
//! private to the process.

/// Why a pane exited. Broadcast to other panes via PaneExited.
/// No error details — a pane's internal failure reason is not
/// broadcast to peers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitReason {
    /// Handler returned Flow::Stop voluntarily.
    Graceful,
    /// Primary connection lost.
    Disconnected,
    /// Handler panicked (caught by catch_unwind).
    Failed,
    /// Infrastructure failure (calloop, socket, framing).
    InfraError,
}
