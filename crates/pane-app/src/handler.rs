use pane_proto::event::{KeyEvent, MouseEvent};
use pane_proto::protocol::PaneGeometry;

use crate::error::Result;
use crate::proxy::PaneHandle;

/// Trait for structured pane event handling.
///
/// Implement the methods you care about; all have defaults that
/// continue the event loop. `close_requested` defaults to accepting
/// the close (returning `Ok(false)` to stop the loop).
///
/// This is the BHandler pattern: override what you understand,
/// ignore the rest. Rust's exhaustive matching via PaneEvent ensures
/// nothing is silently dropped — every variant reaches a Handler
/// method or the `unhandled` catch-all.
///
/// # Return value
///
/// All methods return `Result<bool>`:
/// - `Ok(true)` — continue the event loop
/// - `Ok(false)` — exit the event loop (sends RequestClose)
/// - `Err(e)` — exit the event loop with an error
pub trait Handler: Send + 'static {
    /// Called once when the pane is ready (initial geometry received).
    /// The PaneHandle allows sending messages back to the compositor.
    fn ready(&mut self, _proxy: &PaneHandle, _geometry: PaneGeometry) -> Result<bool> {
        Ok(true)
    }

    /// The pane was resized.
    fn resized(&mut self, _proxy: &PaneHandle, _geometry: PaneGeometry) -> Result<bool> {
        Ok(true)
    }

    /// The pane gained focus.
    fn focused(&mut self, _proxy: &PaneHandle) -> Result<bool> {
        Ok(true)
    }

    /// The pane lost focus.
    fn blurred(&mut self, _proxy: &PaneHandle) -> Result<bool> {
        Ok(true)
    }

    /// Keyboard input.
    fn key(&mut self, _proxy: &PaneHandle, _event: KeyEvent) -> Result<bool> {
        Ok(true)
    }

    /// Mouse input.
    fn mouse(&mut self, _proxy: &PaneHandle, _event: MouseEvent) -> Result<bool> {
        Ok(true)
    }

    /// The command surface was activated.
    fn command_activated(&mut self, _proxy: &PaneHandle) -> Result<bool> {
        Ok(true)
    }

    /// The command surface was dismissed.
    fn command_dismissed(&mut self, _proxy: &PaneHandle) -> Result<bool> {
        Ok(true)
    }

    /// A command was executed from the command surface.
    fn command_executed(&mut self, _proxy: &PaneHandle, _command: &str, _args: &str) -> Result<bool> {
        Ok(true)
    }

    /// The compositor requests completions for the command input.
    fn completion_request(&mut self, _proxy: &PaneHandle, _token: u64, _input: &str) -> Result<bool> {
        Ok(true)
    }

    /// The compositor requests this pane to close.
    /// Default: accept the close (return Ok(false) to stop the loop).
    fn close_requested(&mut self, _proxy: &PaneHandle) -> Result<bool> {
        Ok(false)
    }

    /// The connection to the compositor was lost.
    fn disconnected(&mut self, _proxy: &PaneHandle) -> Result<bool> {
        Ok(false)
    }

    /// Catch-all for events not handled by other methods.
    fn unhandled(&mut self, _proxy: &PaneHandle) -> Result<bool> {
        Ok(true)
    }
}
