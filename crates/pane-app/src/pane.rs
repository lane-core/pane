//! The Pane handle — represents a single pane in the compositor.

use std::sync::mpsc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use pane_proto::message::PaneId;
use pane_proto::protocol::{ClientToComp, PaneGeometry};

use crate::error::Result;
use crate::event::Message;
use crate::exit::ExitReason;
use crate::filter::FilterChain;
use crate::handler::Handler;
use crate::looper;
use crate::looper_message::LooperMessage;
use crate::proxy::{Messenger, LooperSender};

/// Handle to a single pane in the compositor.
///
/// Created by [`App::create_pane`](crate::App::create_pane). A Pane
/// represents one compositor surface with its own event loop, filter
/// chain, and shortcut table.
///
/// Pane is a move-only type: you configure it (add filters,
/// shortcuts), then consume it by calling [`run`](Pane::run) or
/// [`run_with`](Pane::run_with). Once the event loop starts,
/// ongoing communication happens through the [`Messenger`] handle
/// passed to your handler, not through Pane itself.
///
/// # Threading
///
/// The event loop runs on the calling thread. If you need multiple
/// panes, spawn threads.
///
/// # BeOS
///
/// Descends from `BWindow` (see also Haiku's
/// [BWindow documentation](reference/haiku-book/interface/Window.dox)).
/// Key changes:
/// - Not a looper subclass. Pane owns a looper internally but the
///   public API is configuration + run, not direct loop control.
/// - Consumed by `run()` — you cannot call methods on a Pane after
///   the loop starts. Use [`Messenger`] instead.
/// - `Lock()`/`Unlock()` replaced by `&mut self` — the borrow
///   checker is the lock.
pub struct Pane {
    id: PaneId,
    geometry: PaneGeometry,
    receiver: calloop::channel::Channel<LooperMessage>,
    comp_tx: mpsc::Sender<ClientToComp>,
    looper_tx: LooperSender,
    pane_count: Arc<AtomicUsize>,
    done_signal: Arc<(std::sync::Mutex<()>, std::sync::Condvar)>,
    filters: FilterChain,
    shortcuts: crate::shortcuts::ShortcutFilter,
}

impl Pane {
    pub(crate) fn new(
        id: PaneId,
        geometry: PaneGeometry,
        receiver: calloop::channel::Channel<LooperMessage>,
        comp_tx: mpsc::Sender<ClientToComp>,
        looper_tx: LooperSender,
        pane_count: Arc<AtomicUsize>,
        done_signal: Arc<(std::sync::Mutex<()>, std::sync::Condvar)>,
    ) -> Self {
        Pane {
            id,
            geometry,
            receiver,
            comp_tx,
            looper_tx,
            pane_count,
            done_signal,
            filters: FilterChain::new(),
            shortcuts: crate::shortcuts::ShortcutFilter::new(),
        }
    }

    /// The compositor-assigned pane ID.
    pub fn id(&self) -> PaneId {
        self.id
    }

    /// The pane's current geometry.
    pub fn geometry(&self) -> PaneGeometry {
        self.geometry
    }

    /// Add a filter to the pane's filter chain.
    pub fn add_filter(&mut self, filter: impl crate::filter::MessageFilter) {
        self.filters.add(filter);
    }

    /// Register a keyboard shortcut. When the key combo is pressed,
    /// the handler receives `Message::CommandExecuted { command, args }`
    /// instead of the raw key event. BWindow::AddShortcut equivalent.
    pub fn add_shortcut(
        &mut self,
        combo: crate::shortcuts::KeyCombo,
        command: impl Into<String>,
        args: impl Into<String>,
    ) {
        self.shortcuts.add(combo, command, args);
    }

    /// Get a Messenger for this pane. The messenger can be cloned and
    /// sent to other threads for sending messages to the compositor
    /// or back to this pane's own looper.
    pub fn messenger(&self) -> Messenger {
        Messenger::new(self.id, self.comp_tx.clone())
            .with_looper(self.looper_tx.clone())
    }

    /// Run the event loop with a closure handler on the current thread.
    ///
    /// The closure receives a `&Messenger` and a `Message`, and returns:
    /// - `Ok(true)` — continue
    /// - `Ok(false)` — exit (sends RequestClose)
    /// - `Err(e)` — exit with error
    ///
    /// This is the hello-pane pattern:
    /// ```ignore
    /// pane.run(|messenger, msg| match msg {
    ///     Message::Key(key) if key.is_escape() => Ok(false),
    ///     Message::CloseRequested => Ok(false),
    ///     _ => Ok(true),
    /// })
    /// ```
    ///
    /// # BeOS
    ///
    /// `BLooper::Run` + `BWindow::Show`. Be's `Run()` spawned a new
    /// thread; pane's `run()` executes on the calling thread. The
    /// caller spawns if they want a separate thread. `run()` consumes
    /// the `Pane`, matching Be's ownership transfer to the looper thread.
    pub fn run(self, mut handler: impl FnMut(&Messenger, Message) -> Result<bool>) -> Result<()> {
        let Pane { id, geometry, receiver, mut filters, comp_tx, looper_tx, pane_count, done_signal, shortcuts, .. } = self;
        if !shortcuts.is_empty() {
            filters.add(shortcuts);
        }
        let handle = Messenger::new(id, comp_tx.clone()).with_looper(looper_tx);

        // Synthesize Ready as the first message
        let keep_going = handler(&handle, Message::Ready(geometry))?;
        if !keep_going {
            handle.broadcaster.broadcast(id, ExitReason::HandlerExit);
            if pane_count.fetch_sub(1, Ordering::Release) == 1 {
                let _lock = done_signal.0.lock().unwrap_or_else(|e| e.into_inner());
                done_signal.1.notify_all();
            }
            return Ok(());
        }

        let exit = looper::run_closure(id, receiver, filters, handle.clone(), handler)?;

        handle.broadcaster.broadcast(id, exit);

        if exit.should_request_close() {
            let _ = comp_tx.send(ClientToComp::RequestClose { pane: id });
        }
        if pane_count.fetch_sub(1, Ordering::Release) == 1 {
            let _lock = done_signal.0.lock().unwrap_or_else(|e| e.into_inner());
            done_signal.1.notify_all();
        }

        Ok(())
    }

    /// Run the event loop with a Handler trait implementation.
    ///
    /// The weather widget pattern:
    /// ```ignore
    /// pane.run_with(Weather { city: "SF".into(), data: None })
    /// ```
    ///
    /// # BeOS
    ///
    /// Same as [`run`](Pane::run) but dispatches through the
    /// [`Handler`] trait — replaces the pattern of subclassing
    /// `BWindow` and overriding `MessageReceived`, `FrameResized`,
    /// `WindowActivated`, etc.
    pub fn run_with(self, mut handler: impl Handler) -> Result<()> {
        let Pane { id, geometry, receiver, mut filters, comp_tx, looper_tx, pane_count, done_signal, shortcuts, .. } = self;
        if !shortcuts.is_empty() {
            filters.add(shortcuts);
        }
        let handle = Messenger::new(id, comp_tx.clone()).with_looper(looper_tx);

        // Synthesize Ready as the first message
        let keep_going = handler.ready(&handle, geometry)?;
        if !keep_going {
            handle.broadcaster.broadcast(id, ExitReason::HandlerExit);
            if pane_count.fetch_sub(1, Ordering::Release) == 1 {
                let _lock = done_signal.0.lock().unwrap_or_else(|e| e.into_inner());
                done_signal.1.notify_all();
            }
            return Ok(());
        }

        let exit = looper::run_handler(id, receiver, filters, handle.clone(), handler)?;

        handle.broadcaster.broadcast(id, exit);

        if exit.should_request_close() {
            let _ = comp_tx.send(ClientToComp::RequestClose { pane: id });
        }
        if pane_count.fetch_sub(1, Ordering::Release) == 1 {
            let _lock = done_signal.0.lock().unwrap_or_else(|e| e.into_inner());
            done_signal.1.notify_all();
        }

        Ok(())
    }

    // Note: set_title, set_vocabulary, set_content live on Messenger,
    // not Pane. Pane is consumed by run(), so these can't be called
    // during the event loop. Use pane.messenger() before run() or the
    // &Messenger passed to your handler/closure.
}

impl std::fmt::Debug for Pane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pane")
            .field("id", &self.id)
            .field("geometry", &self.geometry)
            .finish()
    }
}
