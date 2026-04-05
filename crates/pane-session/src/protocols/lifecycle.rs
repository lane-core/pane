//! Lifecycle protocol — every pane speaks this.
//!
//! Part of the Control protocol (wire service 0, implicit).
//! Never DeclareInterest'd — exists by virtue of having a connection.
//! Handler provides named methods as sugar; the blanket
//! Handles<Lifecycle> impl dispatches through the same mechanism
//! as all other protocols.

use serde::{Serialize, Deserialize};
use crate::protocol::{Protocol, ServiceId};

/// Lifecycle protocol definition.
pub struct Lifecycle;

impl Protocol for Lifecycle {
    const SERVICE_ID: ServiceId = ServiceId::new("com.pane.lifecycle");
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
}
