//! The per-pane event loop. Internal to the kit.
//!
//! Each pane has its own looper running on its own thread (or the
//! calling thread for `pane.run()`). The looper reads CompToClient
//! messages from a channel, converts them to PaneEvents, applies
//! filters, and dispatches to the handler or closure.
//!
//! This is the BLooper model: one thread, one message queue,
//! sequential processing. Concurrency arises from many loopers
//! running simultaneously, not from concurrency within a looper.

use std::sync::mpsc;

use pane_proto::message::PaneId;
use pane_proto::protocol::CompToClient;

use crate::error::Result;
use crate::event::PaneEvent;
use crate::filter::FilterChain;
use crate::handler::Handler;

/// Run the event loop with a closure handler.
///
/// The closure receives a PaneEvent and returns:
/// - Ok(true) to continue
/// - Ok(false) to exit
/// - Err to exit with error
pub fn run_closure(
    pane_id: PaneId,
    receiver: mpsc::Receiver<CompToClient>,
    mut filters: FilterChain,
    mut handler: impl FnMut(PaneEvent) -> Result<bool>,
) -> Result<()> {
    loop {
        let msg = match receiver.recv() {
            Ok(msg) => msg,
            Err(_) => {
                // Channel closed — compositor or dispatcher died
                let _ = handler(PaneEvent::Disconnected);
                return Ok(());
            }
        };

        let event = match PaneEvent::from_comp(&msg, pane_id) {
            Some(e) => e,
            None => continue, // not for this pane, or internal message
        };

        let event = match filters.apply(event) {
            Some(e) => e,
            None => continue, // consumed by filter
        };

        let keep_going = handler(event)?;
        if !keep_going {
            return Ok(());
        }
    }
}

/// Run the event loop with a Handler trait implementation.
pub fn run_handler(
    pane_id: PaneId,
    receiver: mpsc::Receiver<CompToClient>,
    mut filters: FilterChain,
    mut handler: impl Handler,
) -> Result<()> {
    loop {
        let msg = match receiver.recv() {
            Ok(msg) => msg,
            Err(_) => {
                let _ = handler.disconnected();
                return Ok(());
            }
        };

        let event = match PaneEvent::from_comp(&msg, pane_id) {
            Some(e) => e,
            None => continue,
        };

        let event = match filters.apply(event) {
            Some(e) => e,
            None => continue,
        };

        let keep_going = dispatch_to_handler(&mut handler, event)?;
        if !keep_going {
            return Ok(());
        }
    }
}

/// Dispatch a single PaneEvent to the appropriate Handler method.
fn dispatch_to_handler(handler: &mut impl Handler, event: PaneEvent) -> Result<bool> {
    match event {
        PaneEvent::Ready(geom) => handler.ready(geom),
        PaneEvent::Resize(geom) => handler.resized(geom),
        PaneEvent::Focus => handler.focused(),
        PaneEvent::Blur => handler.blurred(),
        PaneEvent::Key(key) => handler.key(key),
        PaneEvent::Mouse(mouse) => handler.mouse(mouse),
        PaneEvent::Close => handler.close_requested(),
        PaneEvent::CommandActivated => handler.command_activated(),
        PaneEvent::CommandDismissed => handler.command_dismissed(),
        PaneEvent::CommandExecuted { command, args } =>
            handler.command_executed(&command, &args),
        PaneEvent::CompletionRequest { token, input } =>
            handler.completion_request(token, &input),
        PaneEvent::Disconnected => handler.disconnected(),
    }
}
