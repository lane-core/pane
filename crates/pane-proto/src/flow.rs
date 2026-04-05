//! Handler control flow.
//!
//! Returned by every handler method and Handles<P>::receive.
//! Both variants represent normal handler completion — the handler
//! finished processing without panic. Flow::Stop triggers the
//! destruction sequence; Flow::Continue returns to idle.

/// Handler control flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Stop,
}
