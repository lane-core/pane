use pane_proto::event::{KeyEvent, MouseEvent};
use pane_proto::message::PaneId;
use pane_proto::protocol::PaneGeometry;

use crate::error::Result;
use crate::exit::ExitReason;
use crate::proxy::Messenger;

/// Trait for structured pane event handling.
///
/// Implement the methods you care about; all have defaults that
/// continue the event loop. `close_requested` defaults to accepting
/// the close (returning `Ok(false)` to stop the loop).
///
/// Each method corresponds to one [`Message`](crate::Message) variant.
/// The looper dispatches to the matching method automatically, so you
/// never need to write a top-level match yourself — that's what
/// [`Pane::run`](crate::Pane::run)'s closure form is for.
///
/// # Return Convention
///
/// All methods return `Result<bool>`:
/// - `Ok(true)` — continue the event loop
/// - `Ok(false)` — exit cleanly (the compositor is notified)
/// - `Err(e)` — exit with an error
///
/// The default for most methods is `Ok(true)` (keep going).
/// The exception is [`close_requested`](Handler::close_requested),
/// which defaults to `Ok(false)` (accept the close).
///
/// # BeOS
///
/// Descends from `BHandler` (see also Haiku's
/// [BHandler documentation](reference/haiku-book/app/BHandler.dox)).
/// Key changes:
/// - Per-event methods replace single `MessageReceived(BMessage*)`
///   dispatch — exhaustive: every event reaches a method or the
///   `fallback` catch-all
/// - Return value controls the event loop (Be used side-effects
///   like `PostMessage(B_QUIT_REQUESTED)`)
/// - Trait with default methods replaces virtual class hierarchy
pub trait Handler: Send + 'static {
    /// Called once when the pane is ready and its initial geometry is known.
    ///
    /// This is the first event your handler sees. Use it for one-time
    /// setup that depends on geometry: initializing a rendering buffer,
    /// starting a pulse timer, loading initial content.
    ///
    /// Default: continues the event loop (`Ok(true)`).
    fn ready(&mut self, _proxy: &Messenger, _geometry: PaneGeometry) -> Result<bool> {
        Ok(true)
    }

    /// The pane was resized.
    fn resized(&mut self, _proxy: &Messenger, _geometry: PaneGeometry) -> Result<bool> {
        Ok(true)
    }

    /// The pane gained input focus.
    ///
    /// Override to update visual state (cursor style, selection
    /// highlight) or start input capture.
    ///
    /// Default: continues the event loop (`Ok(true)`).
    ///
    /// # BeOS
    ///
    /// `BWindow::WindowActivated` with `active == true`. pane splits
    /// the `bool active` parameter into separate `activated` /
    /// `deactivated` hooks.
    fn activated(&mut self, _proxy: &Messenger) -> Result<bool> {
        Ok(true)
    }

    /// The pane lost input focus.
    ///
    /// Default: continues the event loop (`Ok(true)`).
    ///
    /// # BeOS
    ///
    /// `BWindow::WindowActivated` with `active == false`.
    fn deactivated(&mut self, _proxy: &Messenger) -> Result<bool> {
        Ok(true)
    }

    /// Keyboard input.
    fn key(&mut self, _proxy: &Messenger, _event: KeyEvent) -> Result<bool> {
        Ok(true)
    }

    /// Mouse input.
    fn mouse(&mut self, _proxy: &Messenger, _event: MouseEvent) -> Result<bool> {
        Ok(true)
    }

    /// The command surface was activated.
    fn command_activated(&mut self, _proxy: &Messenger) -> Result<bool> {
        Ok(true)
    }

    /// The command surface was dismissed.
    fn command_dismissed(&mut self, _proxy: &Messenger) -> Result<bool> {
        Ok(true)
    }

    /// A command was executed from the command surface.
    fn command_executed(&mut self, _proxy: &Messenger, _command: &str, _args: &str) -> Result<bool> {
        Ok(true)
    }

    /// The compositor requests completions for the command input.
    fn completion_request(&mut self, _proxy: &Messenger, _token: u64, _input: &str) -> Result<bool> {
        Ok(true)
    }

    /// Periodic pulse. Override for animations, status polling, clock ticks.
    ///
    /// Delivered at the interval set by [`Messenger::set_pulse_rate`].
    /// Not delivered until you set a rate.
    ///
    /// Default: continues the event loop (`Ok(true)`).
    ///
    /// # BeOS
    ///
    /// `BWindow::Pulse`.
    fn pulse(&mut self, _proxy: &Messenger) -> Result<bool> {
        Ok(true)
    }

    /// The compositor requests this pane to close.
    ///
    /// Override to implement save-before-close, confirmation dialogs,
    /// or to refuse the close. Return `Ok(false)` to accept (stop
    /// the loop) or `Ok(true)` to reject and keep running.
    ///
    /// Default: accepts the close (`Ok(false)`).
    ///
    /// # BeOS
    ///
    /// `BWindow::QuitRequested` — same semantics but the return
    /// convention is inverted: Be returned `true` to accept,
    /// pane returns `Ok(false)` to stop the loop.
    fn close_requested(&mut self, _proxy: &Messenger) -> Result<bool> {
        Ok(false)
    }

    /// The connection to the compositor was lost.
    fn disconnected(&mut self, _proxy: &Messenger) -> Result<bool> {
        Ok(false)
    }

    /// A monitored pane exited. Default: continue.
    fn pane_exited(&mut self, _proxy: &Messenger, _pane: PaneId, _reason: ExitReason) -> Result<bool> {
        Ok(true)
    }

    /// Catch-all for events not handled by other methods.
    fn fallback(&mut self, _proxy: &Messenger) -> Result<bool> {
        Ok(true)
    }
}
