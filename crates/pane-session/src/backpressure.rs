//! Backpressure signal for fallible send operations.
//!
//! Returned by `try_send_request` and `try_send_notification` when
//! the send cannot complete without blocking or violating caps.
//! The caller retains ownership of the unsent message (returned
//! inside the error) and can retry, queue, or drop it.
//!
//! This is a runtime type, not a wire type — it is never serialized.
//!
//! Design heritage: Plan 9 had no explicit backpressure — writes
//! to a full mount pipe blocked the calling process (devmnt.c
//! mountio():798). Haiku's port_buffer_size_etc() returned
//! B_WOULD_BLOCK on a full port, but only for non-blocking sends
//! (src/system/kernel/port.cpp:990-1005). pane's Backpressure
//! makes the reason explicit and typed, letting callers
//! distinguish cap exhaustion from channel contention.

/// Why a `try_send_*` could not complete.
///
/// Returned paired with the unsent message so the caller retains
/// ownership. The linearity condition (D1, L2): on the error path,
/// the request obligation must not be consumed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Backpressure {
    /// The negotiated `max_outstanding_requests` cap would be
    /// exceeded. The caller has too many in-flight requests on
    /// this connection.
    CapExceeded,
    /// The write channel is full. Transient — the bridge thread
    /// has not drained pending frames yet.
    ChannelFull,
    /// The connection is closing or already closed. The write
    /// channel has been dropped.
    ConnectionClosing,
}

impl std::fmt::Display for Backpressure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Backpressure::CapExceeded => {
                write!(f, "max_outstanding_requests cap exceeded")
            }
            Backpressure::ChannelFull => write!(f, "write channel full"),
            Backpressure::ConnectionClosing => write!(f, "connection closing"),
        }
    }
}

impl std::error::Error for Backpressure {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_are_distinct() {
        assert_ne!(Backpressure::CapExceeded, Backpressure::ChannelFull);
        assert_ne!(Backpressure::CapExceeded, Backpressure::ConnectionClosing);
        assert_ne!(Backpressure::ChannelFull, Backpressure::ConnectionClosing);
    }

    #[test]
    fn display_messages() {
        assert_eq!(
            Backpressure::CapExceeded.to_string(),
            "max_outstanding_requests cap exceeded"
        );
        assert_eq!(Backpressure::ChannelFull.to_string(), "write channel full");
        assert_eq!(
            Backpressure::ConnectionClosing.to_string(),
            "connection closing"
        );
    }

    #[test]
    fn implements_error_trait() {
        let err: &dyn std::error::Error = &Backpressure::CapExceeded;
        assert!(!err.to_string().is_empty());
    }
}
