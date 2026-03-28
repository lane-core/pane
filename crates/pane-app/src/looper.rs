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
use crate::exit::ExitReason;
use crate::filter::FilterChain;
use crate::handler::Handler;
use crate::proxy::PaneHandle;

/// Run the event loop with a closure handler.
///
/// The closure receives a PaneHandle (for sending messages back to the
/// compositor) and a PaneEvent, and returns:
/// - Ok(true) to continue
/// - Ok(false) to exit
/// - Err to exit with error
pub fn run_closure(
    pane_id: PaneId,
    receiver: mpsc::Receiver<CompToClient>,
    mut filters: FilterChain,
    proxy: PaneHandle,
    mut handler: impl FnMut(&PaneHandle, PaneEvent) -> Result<bool>,
) -> std::result::Result<ExitReason, crate::error::Error> {
    loop {
        let msg = match receiver.recv() {
            Ok(msg) => msg,
            Err(_) => {
                let _ = handler(&proxy, PaneEvent::Disconnected);
                return Ok(ExitReason::Disconnected);
            }
        };

        let event = match PaneEvent::from_comp(msg, pane_id) {
            Some(e) => e,
            None => continue,
        };

        let is_close = matches!(event, PaneEvent::Close);

        let event = match filters.apply(event) {
            Some(e) => e,
            None => continue,
        };

        let keep_going = handler(&proxy, event)?;
        if !keep_going {
            return Ok(if is_close {
                ExitReason::CompositorClose
            } else {
                ExitReason::HandlerExit
            });
        }
    }
}

/// Run the event loop with a Handler trait implementation.
pub fn run_handler(
    pane_id: PaneId,
    receiver: mpsc::Receiver<CompToClient>,
    mut filters: FilterChain,
    proxy: PaneHandle,
    mut handler: impl Handler,
) -> std::result::Result<ExitReason, crate::error::Error> {
    loop {
        let msg = match receiver.recv() {
            Ok(msg) => msg,
            Err(_) => {
                let _ = handler.disconnected(&proxy);
                return Ok(ExitReason::Disconnected);
            }
        };

        let event = match PaneEvent::from_comp(msg, pane_id) {
            Some(e) => e,
            None => continue,
        };

        let is_close = matches!(event, PaneEvent::Close);

        let event = match filters.apply(event) {
            Some(e) => e,
            None => continue,
        };

        let keep_going = dispatch_to_handler(&mut handler, &proxy, event)?;
        if !keep_going {
            return Ok(if is_close {
                ExitReason::CompositorClose
            } else {
                ExitReason::HandlerExit
            });
        }
    }
}

/// Dispatch a single PaneEvent to the appropriate Handler method.
fn dispatch_to_handler(handler: &mut impl Handler, proxy: &PaneHandle, event: PaneEvent) -> Result<bool> {
    match event {
        PaneEvent::Ready(geom) => handler.ready(proxy, geom),
        PaneEvent::Resize(geom) => handler.resized(proxy, geom),
        PaneEvent::Focus => handler.focused(proxy),
        PaneEvent::Blur => handler.blurred(proxy),
        PaneEvent::Key(key) => handler.key(proxy, key),
        PaneEvent::Mouse(mouse) => handler.mouse(proxy, mouse),
        PaneEvent::Close => handler.close_requested(proxy),
        PaneEvent::CommandActivated => handler.command_activated(proxy),
        PaneEvent::CommandDismissed => handler.command_dismissed(proxy),
        PaneEvent::CommandExecuted { command, args } =>
            handler.command_executed(proxy, &command, &args),
        PaneEvent::CompletionRequest { token, input } =>
            handler.completion_request(proxy, token, &input),
        PaneEvent::Disconnected => handler.disconnected(proxy),
    }
}
