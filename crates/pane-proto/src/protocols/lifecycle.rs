//! Lifecycle protocol — every pane speaks this.
//!
//! Part of the Control protocol (wire service 0, implicit).
//! Never DeclareInterest'd — exists by virtue of having a connection.
//! Handler provides named methods as sugar; the blanket
//! Handles<Lifecycle> impl dispatches through the same mechanism
//! as all other protocols.

use crate::address::Address;
use crate::exit_reason::ExitReason;
use crate::protocol::{Protocol, ServiceId};
use serde::{Deserialize, Serialize};

/// Lifecycle protocol definition.
pub struct Lifecycle;

impl Protocol for Lifecycle {
    fn service_id() -> ServiceId {
        ServiceId::new("com.pane.lifecycle")
    }
    type Message = LifecycleMessage;
}

/// Lifecycle events. Clone-safe values only — no obligations.
/// Dispatched through Handler's named methods via blanket impl.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LifecycleMessage {
    Ready,
    CloseRequested,
    Disconnected,
    Pulse,
    /// A watched pane has exited. Dispatches to
    /// [`Handler::pane_exited`](crate::handler::Handler::pane_exited).
    ///
    /// Only fires for panes explicitly watched via
    /// `Messenger::watch()`. Not a broadcast — registration-based.
    ///
    /// # BeOS
    ///
    /// `B_SOME_APP_QUIT` (src/servers/registrar/WatchingService.cpp:204-228).
    PaneExited {
        address: Address,
        reason: ExitReason,
    },
}
