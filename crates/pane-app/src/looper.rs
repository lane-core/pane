//! The per-pane event loop. Internal to the kit.
//!
//! Each pane has its own looper running on its own thread (or the
//! calling thread for `pane.run()`). The looper reads LooperMessages
//! from a unified channel (compositor events + self-delivered events),
//! converts them to Messages, applies filters, and dispatches to
//! the handler or closure.
//!
//! This is the BLooper model: one thread, one message queue,
//! sequential processing. Concurrency arises from many loopers
//! running simultaneously, not from concurrency within a looper.

use std::sync::mpsc;

use pane_proto::event::MouseEventKind;
use pane_proto::message::PaneId;

use crate::error::Result;
use crate::event::Message;
use crate::exit::ExitReason;
use crate::filter::FilterChain;
use crate::handler::Handler;
use crate::looper_message::LooperMessage;
use crate::proxy::Messenger;

/// Unwrap a LooperMessage into a Message.
/// Returns None for compositor messages with wrong pane_id.
fn unwrap_message(msg: LooperMessage, pane_id: PaneId) -> Option<Message> {
    match msg {
        LooperMessage::FromComp(comp_msg) => Message::from_comp(comp_msg, pane_id),
        LooperMessage::Posted(event) => Some(event),
    }
}

/// Drain the channel after a blocking recv, then coalesce.
///
/// Coalescing rules (from Be's BWindow::DispatchMessage):
/// - Resize: keep only the last geometry
/// - MouseMove: keep only the last position
/// - Everything else: deliver in order
fn drain_and_coalesce(
    first: LooperMessage,
    receiver: &mpsc::Receiver<LooperMessage>,
    pane_id: PaneId,
) -> Vec<Message> {
    let mut batch = vec![first];
    while let Ok(more) = receiver.try_recv() {
        batch.push(more);
    }

    let mut events: Vec<Message> = Vec::with_capacity(batch.len());
    let mut last_resize_idx: Option<usize> = None;
    let mut last_mouse_move_idx: Option<usize> = None;

    for msg in batch {
        let event = match unwrap_message(msg, pane_id) {
            Some(e) => e,
            None => continue,
        };

        match &event {
            Message::Resize(_) => {
                if let Some(idx) = last_resize_idx {
                    events[idx] = event;
                } else {
                    last_resize_idx = Some(events.len());
                    events.push(event);
                }
            }
            Message::Mouse(m) if matches!(m.kind, MouseEventKind::Move) => {
                if let Some(idx) = last_mouse_move_idx {
                    events[idx] = event;
                } else {
                    last_mouse_move_idx = Some(events.len());
                    events.push(event);
                }
            }
            _ => {
                events.push(event);
            }
        }
    }

    events
}

/// Run the event loop with a closure handler.
///
/// The closure receives a Messenger (for sending messages back to the
/// compositor or posting events to this looper) and a Message, and returns:
/// - Ok(true) to continue
/// - Ok(false) to exit
/// - Err to exit with error
pub fn run_closure(
    pane_id: PaneId,
    receiver: mpsc::Receiver<LooperMessage>,
    mut filters: FilterChain,
    proxy: Messenger,
    mut handler: impl FnMut(&Messenger, Message) -> Result<bool>,
) -> std::result::Result<ExitReason, crate::error::Error> {
    loop {
        let msg = match receiver.recv() {
            Ok(msg) => msg,
            Err(_) => {
                let _ = handler(&proxy, Message::Disconnected);
                return Ok(ExitReason::Disconnected);
            }
        };

        let batch = drain_and_coalesce(msg, &receiver, pane_id);

        for event in batch {
            let is_close = matches!(event, Message::CloseRequested);

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
}

/// Run the event loop with a Handler trait implementation.
pub fn run_handler(
    pane_id: PaneId,
    receiver: mpsc::Receiver<LooperMessage>,
    mut filters: FilterChain,
    proxy: Messenger,
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

        let batch = drain_and_coalesce(msg, &receiver, pane_id);

        for event in batch {
            let is_close = matches!(event, Message::CloseRequested);

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
}

/// Dispatch a single Message to the appropriate Handler method.
fn dispatch_to_handler(handler: &mut impl Handler, proxy: &Messenger, event: Message) -> Result<bool> {
    match event {
        Message::Ready(geom) => handler.ready(proxy, geom),
        Message::Resize(geom) => handler.resized(proxy, geom),
        Message::Activated => handler.activated(proxy),
        Message::Deactivated => handler.deactivated(proxy),
        Message::Key(key) => handler.key(proxy, key),
        Message::Mouse(mouse) => handler.mouse(proxy, mouse),
        Message::CloseRequested => handler.quit_requested(proxy),
        Message::CommandActivated => handler.command_activated(proxy),
        Message::CommandDismissed => handler.command_dismissed(proxy),
        Message::CommandExecuted { command, args } =>
            handler.command_executed(proxy, &command, &args),
        Message::CompletionRequest { token, input } =>
            handler.completion_request(proxy, token, &input),
        Message::Pulse => handler.pulse(proxy),
        Message::Disconnected => handler.disconnected(proxy),
        Message::PaneExited { pane, reason } => handler.pane_exited(proxy, pane, reason),
    }
}
