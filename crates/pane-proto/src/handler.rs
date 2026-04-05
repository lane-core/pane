//! Handler: lifecycle sugar over Handles<Lifecycle>.
//!
//! Every pane implements Handler. Named lifecycle methods are the
//! ergonomic surface. Internally, Handler is Handles<Lifecycle> via
//! blanket impl — one dispatch mechanism for everything.
//!
//! BeOS: BLooper::QuitRequested() → bool, BHandler::MessageReceived()
//! → void. pane's Handler returns Flow from every method (except
//! quit_requested which is a bool query, not dispatch).

use crate::flow::Flow;
use crate::handles::Handles;
use crate::protocols::lifecycle::{Lifecycle, LifecycleMessage};

/// Every pane implements this. Lifecycle + messaging.
///
/// The zero-cost on-ramp: override what you need, defaults handle
/// the rest. Internally equivalent to Handles<Lifecycle> via blanket
/// impl — the looper dispatches lifecycle through the same fn-pointer
/// mechanism as all other protocols.
pub trait Handler: Send + 'static {
    fn ready(&mut self) -> Flow { Flow::Continue }
    fn close_requested(&mut self) -> Flow { Flow::Stop }
    fn disconnected(&mut self) -> Flow { Flow::Stop }
    fn pulse(&mut self) -> Flow { Flow::Continue }

    /// Query, not dispatch — returns bool, not Flow. &self for
    /// deadlock freedom. Side effects must happen before returning
    /// true (save in close_requested, not here).
    /// BeOS: BLooper::QuitRequested() → bool.
    fn quit_requested(&self) -> bool { true }
}

/// Framework-provided blanket: every Handler automatically implements
/// Handles<Lifecycle>. The looper dispatches LifecycleMessage through
/// the same fn-pointer path as Display, Clipboard, etc.
impl<H: Handler> Handles<Lifecycle> for H {
    fn receive(&mut self, msg: LifecycleMessage) -> Flow {
        match msg {
            LifecycleMessage::Ready => self.ready(),
            LifecycleMessage::CloseRequested => self.close_requested(),
            LifecycleMessage::Disconnected => self.disconnected(),
            LifecycleMessage::Pulse => self.pulse(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MinimalHandler;
    impl Handler for MinimalHandler {}

    struct CustomHandler {
        ready_count: usize,
    }
    impl Handler for CustomHandler {
        fn ready(&mut self) -> Flow {
            self.ready_count += 1;
            Flow::Continue
        }
        fn close_requested(&mut self) -> Flow {
            Flow::Stop
        }
    }

    #[test]
    fn default_handler_returns_expected_flows() {
        let mut h = MinimalHandler;
        assert_eq!(h.ready(), Flow::Continue);
        assert_eq!(h.close_requested(), Flow::Stop);
        assert_eq!(h.disconnected(), Flow::Stop);
        assert_eq!(h.pulse(), Flow::Continue);
        assert!(h.quit_requested());
    }

    #[test]
    fn blanket_handles_lifecycle() {
        let mut h = CustomHandler { ready_count: 0 };

        // Dispatch through Handles<Lifecycle> — same as calling h.ready()
        let flow = <CustomHandler as Handles<Lifecycle>>::receive(
            &mut h,
            LifecycleMessage::Ready,
        );
        assert_eq!(flow, Flow::Continue);
        assert_eq!(h.ready_count, 1);

        // CloseRequested through blanket
        let flow = <CustomHandler as Handles<Lifecycle>>::receive(
            &mut h,
            LifecycleMessage::CloseRequested,
        );
        assert_eq!(flow, Flow::Stop);
    }

    #[test]
    fn lifecycle_dispatch_is_exhaustive() {
        // This test verifies that every LifecycleMessage variant
        // is handled by the blanket impl. If a variant is added
        // to LifecycleMessage without updating the blanket, this
        // file won't compile (exhaustive match).
        let mut h = MinimalHandler;
        for msg in [
            LifecycleMessage::Ready,
            LifecycleMessage::CloseRequested,
            LifecycleMessage::Disconnected,
            LifecycleMessage::Pulse,
        ] {
            let _ = <MinimalHandler as Handles<Lifecycle>>::receive(&mut h, msg);
        }
    }
}
