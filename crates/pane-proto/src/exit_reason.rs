//! Exit disposition for pane death notification.
//!
//! ExitReason is wire-transmitted in PaneExited notifications, so
//! it lives in pane-proto with Serialize + Deserialize. Stripped
//! of error details -- a pane's internal failure reason is not
//! broadcast to peers.
//!
//! Design heritage: BeOS B_SOME_APP_QUIT carried identity fields
//! (team, signature) through WatchingService::NotifyWatchers()
//! (src/servers/registrar/WatchingService.cpp:204-228) but not
//! a structured exit reason. pane adds ExitReason so watchers can
//! distinguish graceful shutdown from crashes.

use serde::{Deserialize, Serialize};

/// Why a pane exited. Broadcast to watchers via PaneExited.
/// No error details -- a pane's internal failure reason is not
/// broadcast to peers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
