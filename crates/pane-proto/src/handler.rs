//! Lifecycle handler trait.
//!
//! Every pane implements Handler. Named lifecycle methods with
//! defaults provide the ergonomic surface. A blanket impl maps
//! Handler to Handles<Lifecycle>, so lifecycle dispatch uses the
//! same mechanism as all other protocols.
//!
//! Design heritage: BeOS BHandler::MessageReceived returned void;
//! pane returns Flow. BLooper::QuitRequested returned bool; pane
//! keeps this as quit_requested(&self) -> bool.

use crate::address::Address;
use crate::control::TeardownReason;
use crate::exit_reason::ExitReason;
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
    fn ready(&mut self) -> Flow {
        Flow::Continue
    }
    fn close_requested(&mut self) -> Flow {
        Flow::Stop
    }
    fn disconnected(&mut self) -> Flow {
        Flow::Stop
    }
    fn pulse(&mut self) -> Flow {
        Flow::Continue
    }

    /// Called when a watched pane has exited. Override to react
    /// to monitored pane death.
    ///
    /// Only fires for panes explicitly watched via
    /// `Messenger::watch()`. Not a broadcast -- registration-based.
    ///
    /// Default: continues the event loop (`Flow::Continue`).
    ///
    /// # BeOS
    ///
    /// `B_SOME_APP_QUIT` delivered through
    /// `WatchingService::NotifyWatchers()`
    /// (src/servers/registrar/WatchingService.cpp:204-228).
    fn pane_exited(&mut self, _pane: Address, _reason: ExitReason) -> Flow {
        Flow::Continue
    }

    /// Called when a subscriber connects to a service this pane
    /// provides. The session_id identifies the subscriber's
    /// service session.
    ///
    /// Override to track subscribers for push-based notification
    /// patterns — construct a `SubscriberSender<P>` via
    /// `Messenger::subscriber_sender()` and store it.
    ///
    /// Default: continues the event loop.
    ///
    /// # Haiku
    ///
    /// `WatchingService` registered watchers on `InterestAccepted`
    /// (src/servers/registrar/WatchingService.cpp:66-228).
    ///
    /// # Plan 9
    ///
    /// The plumber's `open` on a port file registered the fid as
    /// a subscriber (reference/plan9/man/4/plumber). pane's
    /// `DeclareInterest` is the `open` equivalent.
    fn subscriber_connected(&mut self, _session_id: u16) -> Flow {
        Flow::Continue
    }

    /// Called when a subscriber disconnects from a provided service
    /// (revoked interest or connection lost).
    ///
    /// Override to clean up subscriber state (e.g., remove a
    /// stored `SubscriberSender`). The reason distinguishes
    /// voluntary revocation (`ServiceRevoked`) from connection
    /// failure (`ConnectionLost`).
    ///
    /// Default: continues the event loop.
    ///
    /// # Haiku
    ///
    /// `WatchingService` detected dead watchers reactively on
    /// send failure and cleaned up
    /// (src/servers/registrar/Watcher.cpp:56-93). pane notifies
    /// the provider proactively via this callback.
    fn subscriber_disconnected(&mut self, _session_id: u16, _reason: TeardownReason) -> Flow {
        Flow::Continue
    }

    /// Query, not dispatch — returns bool, not Flow. &self for
    /// deadlock freedom. Side effects must happen before returning
    /// true (save in close_requested, not here).
    /// BeOS: BLooper::QuitRequested() → bool.
    fn quit_requested(&self) -> bool {
        true
    }
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
            LifecycleMessage::PaneExited { address, reason } => self.pane_exited(address, reason),
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
        assert_eq!(
            h.pane_exited(crate::Address::local(1), crate::ExitReason::Graceful,),
            Flow::Continue,
        );
        assert_eq!(h.subscriber_connected(1), Flow::Continue);
        assert_eq!(
            h.subscriber_disconnected(1, TeardownReason::ServiceRevoked),
            Flow::Continue,
        );
        assert!(h.quit_requested());
    }

    #[test]
    fn blanket_handles_lifecycle() {
        let mut h = CustomHandler { ready_count: 0 };

        // Dispatch through Handles<Lifecycle> — same as calling h.ready()
        let flow = <CustomHandler as Handles<Lifecycle>>::receive(&mut h, LifecycleMessage::Ready);
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
            LifecycleMessage::PaneExited {
                address: crate::Address::local(1),
                reason: crate::ExitReason::Graceful,
            },
        ] {
            let _ = <MinimalHandler as Handles<Lifecycle>>::receive(&mut h, msg);
        }
    }
}
