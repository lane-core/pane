//! The Message trait — universal contract for protocol messages.
//!
//! Every Protocol::Message implements Message. Recovers what BMessage
//! was (the common currency of the system) but typed. Clone is
//! required: messages may be filtered, logged, projected into the
//! namespace. Obligation handles are NOT Message types — they are
//! delivered via separate callbacks.

use serde::{de::DeserializeOwned, Serialize};

/// The universal message contract.
///
/// Marker trait — the bounds ARE the contract. No methods.
/// Clone: messages may be filtered, logged, projected, forwarded.
/// Serialize + DeserializeOwned: messages cross process boundaries.
/// Send + 'static: messages are dispatched across thread boundaries.
///
/// Obligation handles (ReplyPort, ClipboardWriteLock) do NOT implement
/// Message — they are !Clone and !Serialize (they contain channel
/// senders). They are delivered via separate typed callbacks.
pub trait Message: Serialize + DeserializeOwned + Clone + Send + 'static {}

// Blanket impl: any type satisfying the bounds is a Message.
impl<T> Message for T where T: Serialize + DeserializeOwned + Clone + Send + 'static {}
