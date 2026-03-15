use serde::{Deserialize, Serialize};

/// Inter-server message verb. The typed core of `PaneMessage<ServerVerb>`.
///
/// The verb indicates intent; the attrs bag carries the payload.
/// Type safety is recovered via typed view/builder patterns in per-server kit modules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerVerb {
    /// Request information.
    Query,
    /// Notify of an event.
    Notify,
    /// Request an action.
    Command,
}

use crate::polarity::Value;
impl Value for ServerVerb {}
