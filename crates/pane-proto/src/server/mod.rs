pub mod views;
pub mod route;
pub mod roster;

use serde::{Deserialize, Serialize};

use crate::polarity::Value;

/// Inter-server message verb. The typed core of `PaneMessage<ServerVerb>`.
///
/// The verb indicates intent; the attrs bag carries the payload.
/// Type safety is recovered via typed view/builder patterns in sub-modules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerVerb {
    /// Request information.
    Query,
    /// Notify of an event.
    Notify,
    /// Request an action.
    Command,
}

impl Value for ServerVerb {}
