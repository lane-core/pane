/// Why the pane's event loop exited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    /// Handler returned Ok(false) voluntarily (e.g., user pressed Escape).
    /// The kit should send RequestClose to the compositor.
    HandlerExit,
    /// Handler returned Ok(false) in response to PaneEvent::Close.
    /// The compositor already knows — don't send RequestClose.
    CompositorClose,
    /// The connection to the compositor was lost.
    /// Can't send anything — the channel is dead.
    Disconnected,
}

impl ExitReason {
    /// Should the kit send RequestClose to the compositor?
    pub fn should_request_close(&self) -> bool {
        matches!(self, ExitReason::HandlerExit)
    }
}
