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
use crate::proxy::Messenger;

/// Handle to a pane in the compositor.
///
/// Created by `App::create_pane()`. Use `run()` to enter the event
/// loop on the current thread, or `run_with()` to use a Handler.
pub struct Pane {
    id: PaneId,
    geometry: PaneGeometry,
    receiver: mpsc::Receiver<LooperMessage>,
    comp_tx: mpsc::Sender<ClientToComp>,
    looper_tx: mpsc::SyncSender<LooperMessage>,
    pane_count: Arc<AtomicUsize>,
    done_signal: Arc<(std::sync::Mutex<()>, std::sync::Condvar)>,
    filters: FilterChain,
}

impl Pane {
    pub(crate) fn new(
        id: PaneId,
        geometry: PaneGeometry,
        receiver: mpsc::Receiver<LooperMessage>,
        comp_tx: mpsc::Sender<ClientToComp>,
        looper_tx: mpsc::SyncSender<LooperMessage>,
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
    pub fn add_filter(&mut self, filter: impl crate::filter::Filter) {
        self.filters.add(filter);
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
    ///     Message::Close => Ok(false),
    ///     _ => Ok(true),
    /// })
    /// ```
    pub fn run(self, mut handler: impl FnMut(&Messenger, Message) -> Result<bool>) -> Result<()> {
        let Pane { id, geometry, receiver, filters, comp_tx, looper_tx, pane_count, done_signal, .. } = self;
        let handle = Messenger::new(id, comp_tx.clone()).with_looper(looper_tx);

        // Synthesize Ready as the first message
        let keep_going = handler(&handle, Message::Ready(geometry))?;
        if !keep_going {
            handle.broadcaster.broadcast(id, ExitReason::HandlerExit);
            pane_count.fetch_sub(1, Ordering::Relaxed);
            if pane_count.load(Ordering::Relaxed) == 0 {
                done_signal.1.notify_all();
            }
            return Ok(());
        }

        let exit = looper::run_closure(id, receiver, filters, handle.clone(), handler)?;

        handle.broadcaster.broadcast(id, exit);

        if exit.should_request_close() {
            let _ = comp_tx.send(ClientToComp::RequestClose { pane: id });
        }
        if pane_count.fetch_sub(1, Ordering::Relaxed) == 1 {
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
    pub fn run_with(self, mut handler: impl Handler) -> Result<()> {
        let Pane { id, geometry, receiver, filters, comp_tx, looper_tx, pane_count, done_signal, .. } = self;
        let handle = Messenger::new(id, comp_tx.clone()).with_looper(looper_tx);

        // Synthesize Ready as the first message
        let keep_going = handler.ready(&handle, geometry)?;
        if !keep_going {
            handle.broadcaster.broadcast(id, ExitReason::HandlerExit);
            pane_count.fetch_sub(1, Ordering::Relaxed);
            if pane_count.load(Ordering::Relaxed) == 0 {
                done_signal.1.notify_all();
            }
            return Ok(());
        }

        let exit = looper::run_handler(id, receiver, filters, handle.clone(), handler)?;

        handle.broadcaster.broadcast(id, exit);

        if exit.should_request_close() {
            let _ = comp_tx.send(ClientToComp::RequestClose { pane: id });
        }
        if pane_count.fetch_sub(1, Ordering::Relaxed) == 1 {
            done_signal.1.notify_all();
        }

        Ok(())
    }

    // Note: set_title, set_vocabulary, set_content live on Messenger,
    // not Pane. Pane is consumed by run(), so these can't be called
    // during the event loop. Use pane.messenger() before run() or the
    // &Messenger passed to your handler/closure.
}
