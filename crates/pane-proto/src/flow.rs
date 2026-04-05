//! Flow: handler control flow.
//!
//! EAct E-Reset: the handler returns control to the looper with a
//! lifecycle decision. Both variants are E-Reset (the handler
//! completed without panic). Flow::Stop triggers the destruction
//! sequence; Flow::Continue returns to idle for the next event.

/// Handler control flow. Returned by every handler method and
/// every Handles<P>::receive dispatch.
///
/// No Result — errors are the handler's domain (handle internally
/// or panic). The looper receives lifecycle decisions, not errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Stop,
}
