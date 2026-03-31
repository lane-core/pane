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

use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use pane_proto::event::MouseEventKind;
use pane_proto::message::PaneId;

use crate::error::Result;
use crate::event::Message;
use crate::exit::ExitReason;
use crate::filter::FilterChain;
use crate::handler::Handler;
use crate::looper_message::LooperMessage;
use crate::proxy::Messenger;

thread_local! {
    /// True when the current thread is running a looper event loop.
    /// Checked by `send_and_wait` to prevent deadlocks.
    static IS_LOOPER: Cell<bool> = const { Cell::new(false) };
}

/// Returns true if the calling thread is currently running a looper.
pub(crate) fn is_looper_thread() -> bool {
    IS_LOOPER.with(|flag| flag.get())
}

/// RAII guard that clears IS_LOOPER on drop (including panic unwind).
struct LooperGuard;
impl Drop for LooperGuard {
    fn drop(&mut self) {
        IS_LOOPER.with(|flag| flag.set(false));
    }
}

/// A pending periodic or one-shot timer, local to the looper thread.
struct TimerEntry {
    next_fire: Instant,
    /// None = one-shot, Some = periodic.
    interval: Option<Duration>,
    /// How to produce the event. Periodic timers use a factory closure
    /// (called each fire, no Clone needed). One-shots store the event
    /// directly in `one_shot_event` and this is None.
    make_event: Option<Box<dyn Fn() -> Message + Send>>,
    /// The event for one-shot timers (consumed on fire).
    one_shot_event: Option<Message>,
    /// For periodic timers: the cancellation flag shared with TimerToken.
    /// One-shot timers have no cancellation (they fire once and are removed).
    cancelled: Option<Arc<AtomicBool>>,
}

/// Timer state for the looper. Simple sorted vec — panes rarely have
/// more than a handful of timers.
struct Timers {
    entries: Vec<TimerEntry>,
}

impl Timers {
    fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Add a periodic timer with a factory closure.
    fn add_periodic(
        &mut self,
        make_event: Box<dyn Fn() -> Message + Send>,
        interval: Duration,
        cancelled: Arc<AtomicBool>,
    ) {
        self.entries.push(TimerEntry {
            next_fire: Instant::now() + interval,
            interval: Some(interval),
            make_event: Some(make_event),
            one_shot_event: None,
            cancelled: Some(cancelled),
        });
    }

    /// Add a one-shot delayed event (consumed on fire, no Clone needed).
    fn add_one_shot(&mut self, event: Message, fire_at: Instant) {
        self.entries.push(TimerEntry {
            next_fire: fire_at,
            interval: None,
            make_event: None,
            one_shot_event: Some(event),
            cancelled: None,
        });
    }

    /// Time until the next timer fires. None if no timers.
    fn next_timeout(&self) -> Option<Duration> {
        self.entries
            .iter()
            .map(|e| e.next_fire)
            .min()
            .map(|t| t.saturating_duration_since(Instant::now()))
    }

    /// Fire all due timers, returning their events. Removes one-shots
    /// and cancelled entries. Reschedules periodics.
    fn fire_due(&mut self) -> Vec<Message> {
        let now = Instant::now();
        let mut fired = Vec::new();

        self.entries.retain_mut(|entry| {
            // Remove cancelled periodic timers
            if let Some(ref flag) = entry.cancelled {
                if flag.load(Ordering::Acquire) {
                    return false;
                }
            }

            if entry.next_fire <= now {
                // Produce the event: factory for periodic, take for one-shot
                if let Some(ref make) = entry.make_event {
                    fired.push(make());
                } else if let Some(event) = entry.one_shot_event.take() {
                    fired.push(event);
                }

                match entry.interval {
                    Some(interval) => {
                        entry.next_fire = now + interval;
                        true
                    }
                    None => false, // One-shot: remove
                }
            } else {
                true
            }
        });

        fired
    }
}

/// State shared with the calloop event loop callbacks.
struct LooperState {
    /// Messages accumulated during the current dispatch cycle.
    batch: Vec<LooperMessage>,
    /// Set when all channel senders are dropped.
    disconnected: bool,
}

/// Process a LooperMessage that may be a timer control message.
/// Returns true if it was a timer message (handled), false if it
/// needs normal dispatch.
/// Process control messages (timer and filter registration).
/// Consumes the message if it's a control message; returns Some
/// for messages that need normal dispatch.
fn try_handle_control(
    msg: LooperMessage,
    timers: &mut Timers,
    filters: &mut crate::filter::FilterChain,
) -> Option<LooperMessage> {
    match msg {
        LooperMessage::AddTimer { make_event, interval, cancelled, .. } => {
            timers.add_periodic(make_event, interval, cancelled);
            None
        }
        LooperMessage::AddOneShot { event, fire_at } => {
            timers.add_one_shot(event, fire_at);
            None
        }
        LooperMessage::AddFilter { id, filter } => {
            filters.add_with_id(id, filter);
            None
        }
        LooperMessage::RemoveFilter { id } => {
            filters.remove_by_id(id);
            None
        }
        other => Some(other),
    }
}

use crate::reply::ReplyPort;

/// Result of unwrapping a LooperMessage.
enum Unwrapped {
    /// A normal event for dispatch.
    Event(Message),
    /// A request that expects a reply.
    Request(Message, ReplyPort),
    /// App-level quit negotiation — handler must respond.
    QuitRequested(mpsc::Sender<bool>),
    /// Imperative quit — exit immediately.
    Quit,
    /// Timer control or wrong-pane message — skip.
    Skip,
}

/// Unwrap a LooperMessage for dispatch.
fn unwrap_message(
    msg: LooperMessage,
    pane_id: PaneId,
    comp_sender: &mpsc::Sender<pane_proto::protocol::ClientToComp>,
) -> Unwrapped {
    match msg {
        LooperMessage::FromComp(comp_msg) => {
            match Message::try_from_comp(comp_msg, pane_id, comp_sender) {
                Some(event) => Unwrapped::Event(event),
                None => Unwrapped::Skip,
            }
        }
        LooperMessage::Posted(event) => Unwrapped::Event(event),
        LooperMessage::Request(msg, reply) => Unwrapped::Request(msg, reply),
        LooperMessage::AddTimer { .. } | LooperMessage::AddOneShot { .. }
        | LooperMessage::AddFilter { .. } | LooperMessage::RemoveFilter { .. } => Unwrapped::Skip,
        LooperMessage::QuitRequested { response_tx } => Unwrapped::QuitRequested(response_tx),
        LooperMessage::Quit => Unwrapped::Quit,
    }
}

/// Coalesce a batch of LooperMessages.
///
/// Timer control messages (AddTimer, AddOneShot, AddFilter, RemoveFilter)
/// are extracted and registered with the timer/filter state. Everything
/// else is coalesced.
///
/// Coalescing rules (from Be's BWindow::DispatchMessage):
/// - Resize: keep only the last geometry
/// - MouseMove: keep only the last position
/// - Everything else: deliver in order
fn coalesce_batch(
    batch: Vec<LooperMessage>,
    pane_id: PaneId,
    comp_sender: &std::sync::mpsc::Sender<pane_proto::protocol::ClientToComp>,
    timers: &mut Timers,
    filters: &mut crate::filter::FilterChain,
) -> Vec<Unwrapped> {
    let mut events: Vec<Unwrapped> = Vec::with_capacity(batch.len());
    let mut last_resize_idx: Option<usize> = None;
    let mut last_mouse_move_idx: Option<usize> = None;

    for msg in batch {
        // Control messages (timer/filter registration) are consumed here
        let msg = match try_handle_control(msg, timers, filters) {
            Some(msg) => msg, // not a control message, continue processing
            None => continue, // control message consumed
        };

        match unwrap_message(msg, pane_id, comp_sender) {
            Unwrapped::Event(event) => {
                match &event {
                    Message::Resize(_) => {
                        if let Some(idx) = last_resize_idx {
                            events[idx] = Unwrapped::Event(event);
                        } else {
                            last_resize_idx = Some(events.len());
                            events.push(Unwrapped::Event(event));
                        }
                    }
                    Message::Mouse(m) if matches!(m.kind, MouseEventKind::Move) => {
                        if let Some(idx) = last_mouse_move_idx {
                            events[idx] = Unwrapped::Event(event);
                        } else {
                            last_mouse_move_idx = Some(events.len());
                            events.push(Unwrapped::Event(event));
                        }
                    }
                    _ => {
                        events.push(Unwrapped::Event(event));
                    }
                }
            }
            req @ Unwrapped::Request(_, _) => {
                // Requests are never coalesced — always delivered
                events.push(req);
            }
            quit @ Unwrapped::QuitRequested(_) => events.push(quit),
            quit @ Unwrapped::Quit => events.push(quit),
            Unwrapped::Skip => {}
        }
    }

    // Priority scan: when Close is present in a large batch, truncate
    // the batch after Close so subsequent events don't delay teardown.
    // Small batches are left in FIFO order — reordering would skip
    // events that arrived before Close.
    //
    // This matches the Be engineer's recommendation: the real fix for
    // input floods is compositor-side coalescing (server never sends
    // 1000 mouse events). Client-side, we just prevent *more* events
    // from queueing behind Close.
    if events.len() > 16 {
        if let Some(pos) = events.iter().position(|e| {
            matches!(e, Unwrapped::Event(Message::CloseRequested))
        }) {
            events.truncate(pos + 1);
        }
    }

    events
}

/// Convert calloop errors to io::Error for our Error type.
fn calloop_err(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
}

/// Drain all remaining ready messages from the calloop event loop.
///
/// calloop's channel delivers at most 1024 messages per dispatch
/// (MAX_EVENTS_CHECK). When the looper has a burst of messages
/// (e.g., resize flood), we need to drain all of them before
/// coalescing so the batch is complete.
fn drain_channel(
    event_loop: &mut calloop::EventLoop<'_, LooperState>,
    state: &mut LooperState,
) -> std::result::Result<(), crate::error::Error> {
    loop {
        let len = state.batch.len();
        event_loop.dispatch(Some(Duration::ZERO), state).map_err(calloop_err)?;
        if state.batch.len() == len {
            break;
        }
    }
    Ok(())
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
    channel: calloop::channel::Channel<LooperMessage>,
    mut filters: FilterChain,
    proxy: Messenger,
    mut handler: impl FnMut(&Messenger, Message) -> Result<bool>,
) -> std::result::Result<ExitReason, crate::error::Error> {
    IS_LOOPER.with(|flag| flag.set(true));
    let _guard = LooperGuard;
    let mut timers = Timers::new();

    let mut event_loop: calloop::EventLoop<'_, LooperState> =
        calloop::EventLoop::try_new().map_err(calloop_err)?;
    let handle = event_loop.handle();

    handle.insert_source(channel, |event, _, state| {
        match event {
            calloop::channel::Event::Msg(msg) => state.batch.push(msg),
            calloop::channel::Event::Closed => state.disconnected = true,
        }
    }).map_err(calloop_err)?;

    let mut state = LooperState { batch: Vec::new(), disconnected: false };

    loop {
        let timeout = timers.next_timeout();
        event_loop.dispatch(timeout, &mut state).map_err(calloop_err)?;

        // calloop's channel delivers at most 1024 messages per dispatch.
        // Drain remaining without blocking to match the old try_recv loop.
        drain_channel(&mut event_loop, &mut state)?;

        let batch = std::mem::take(&mut state.batch);
        let disconnected = state.disconnected;

        // Don't fire timers after disconnect (matches pre-calloop behavior:
        // disconnect exits without processing pending timers)
        let timer_events = if !disconnected {
            timers.fire_due()
        } else {
            Vec::new()
        };

        let mut events = coalesce_batch(batch, pane_id, &proxy.sender, &mut timers, &mut filters);

        if !timer_events.is_empty() {
            let mut all: Vec<Unwrapped> = timer_events.into_iter()
                .map(Unwrapped::Event)
                .collect();
            all.append(&mut events);
            events = all;
        }

        for item in events {
            match item {
                Unwrapped::Event(event) => {
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
                Unwrapped::Request(msg, reply_port) => {
                    // For closure handlers, requests are delivered as
                    // regular messages. The reply port is dropped (sends
                    // ReplyFailed). Use run_handler for request support.
                    let keep_going = handler(&proxy, msg)?;
                    drop(reply_port);
                    if !keep_going {
                        return Ok(ExitReason::HandlerExit);
                    }
                }
                Unwrapped::QuitRequested(response_tx) => {
                    // Closure handlers always allow quit
                    let _ = response_tx.send(true);
                }
                Unwrapped::Quit => {
                    return Ok(ExitReason::HandlerExit);
                }
                Unwrapped::Skip => {}
            }
        }

        if disconnected {
            let _ = handler(&proxy, Message::Disconnected);
            return Ok(ExitReason::Disconnected);
        }
    }
}

/// Run the event loop with a Handler trait implementation.
pub fn run_handler(
    pane_id: PaneId,
    channel: calloop::channel::Channel<LooperMessage>,
    mut filters: FilterChain,
    proxy: Messenger,
    mut handler: impl Handler,
) -> std::result::Result<ExitReason, crate::error::Error> {
    IS_LOOPER.with(|flag| flag.set(true));
    let _guard = LooperGuard;
    let mut timers = Timers::new();

    let mut event_loop: calloop::EventLoop<'_, LooperState> =
        calloop::EventLoop::try_new().map_err(calloop_err)?;
    let handle = event_loop.handle();

    handle.insert_source(channel, |event, _, state| {
        match event {
            calloop::channel::Event::Msg(msg) => state.batch.push(msg),
            calloop::channel::Event::Closed => state.disconnected = true,
        }
    }).map_err(calloop_err)?;

    let mut state = LooperState { batch: Vec::new(), disconnected: false };

    loop {
        let timeout = timers.next_timeout();
        event_loop.dispatch(timeout, &mut state).map_err(calloop_err)?;
        drain_channel(&mut event_loop, &mut state)?;

        let batch = std::mem::take(&mut state.batch);
        let disconnected = state.disconnected;

        let timer_events = if !disconnected {
            timers.fire_due()
        } else {
            Vec::new()
        };

        let mut events = coalesce_batch(batch, pane_id, &proxy.sender, &mut timers, &mut filters);

        if !timer_events.is_empty() {
            let mut all: Vec<Unwrapped> = timer_events.into_iter()
                .map(Unwrapped::Event)
                .collect();
            all.append(&mut events);
            events = all;
        }

        for item in events {
            match item {
                Unwrapped::Event(event) => {
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
                Unwrapped::Request(msg, reply_port) => {
                    let keep_going = handler.request_received(&proxy, msg, reply_port)?;
                    if !keep_going {
                        return Ok(ExitReason::HandlerExit);
                    }
                }
                Unwrapped::QuitRequested(response_tx) => {
                    let allow = handler.quit_requested();
                    let _ = response_tx.send(allow);
                }
                Unwrapped::Quit => {
                    return Ok(ExitReason::HandlerExit);
                }
                Unwrapped::Skip => {}
            }
        }

        if disconnected {
            let _ = handler.disconnected(&proxy);
            return Ok(ExitReason::Disconnected);
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
        Message::CloseRequested => handler.close_requested(proxy),
        Message::CommandActivated => handler.command_activated(proxy),
        Message::CommandDismissed => handler.command_dismissed(proxy),
        Message::CommandExecuted { command, args } =>
            handler.command_executed(proxy, &command, &args),
        Message::CompletionRequest { input, reply } =>
            handler.completion_request(proxy, &input, reply),
        Message::Pulse => handler.pulse(proxy),
        Message::Disconnected => handler.disconnected(proxy),
        Message::PaneExited { pane, reason } => handler.pane_exited(proxy, pane, reason),
        Message::AppMessage(payload) => handler.app_message(proxy, payload),
        Message::Reply { token, payload } => handler.reply_received(proxy, token, payload),
        Message::ReplyFailed { token } => handler.reply_failed(proxy, token),
        Message::ClipboardLockGranted(lock) =>
            handler.clipboard_lock_granted(proxy, lock),
        Message::ClipboardLockDenied { ref clipboard, ref reason } =>
            handler.clipboard_lock_denied(proxy, clipboard, reason),
        Message::ClipboardChanged { ref clipboard, source } =>
            handler.clipboard_changed(proxy, clipboard, source),
    }
}
