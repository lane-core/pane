//! The Pane handle — represents a single pane in the compositor.

use std::sync::mpsc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use pane_proto::message::PaneId;
use pane_proto::protocol::{ClientToComp, CompToClient, PaneGeometry};
use pane_proto::tag::{PaneTitle, CommandVocabulary};

use crate::error::{PaneError, Result};
use crate::event::PaneEvent;
use crate::filter::FilterChain;
use crate::handler::Handler;
use crate::looper;
use crate::proxy::PaneHandle;

/// Handle to a pane in the compositor.
///
/// Created by `App::create_pane()`. Use `run()` to enter the event
/// loop on the current thread, or `run_with()` to use a Handler.
pub struct Pane {
    id: PaneId,
    geometry: PaneGeometry,
    receiver: mpsc::Receiver<CompToClient>,
    comp_tx: mpsc::Sender<ClientToComp>,
    pane_count: Arc<AtomicUsize>,
    filters: FilterChain,
}

impl Pane {
    pub(crate) fn new(
        id: PaneId,
        geometry: PaneGeometry,
        receiver: mpsc::Receiver<CompToClient>,
        comp_tx: mpsc::Sender<ClientToComp>,
        pane_count: Arc<AtomicUsize>,
    ) -> Self {
        Pane {
            id,
            geometry,
            receiver,
            comp_tx,
            pane_count,
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

    /// Run the event loop with a closure handler on the current thread.
    ///
    /// The closure receives a `PaneEvent` and returns:
    /// - `Ok(true)` — continue
    /// - `Ok(false)` — exit (sends RequestClose)
    /// - `Err(e)` — exit with error
    ///
    /// This is the hello-pane pattern:
    /// ```ignore
    /// pane.run(|event| match event {
    ///     PaneEvent::Key(key) if key.is_escape() => Ok(false),
    ///     PaneEvent::Close => Ok(false),
    ///     _ => Ok(true),
    /// })
    /// ```
    /// Get a PaneHandle for this pane. The proxy can be cloned and
    /// sent to other threads for sending messages to the compositor.
    pub fn proxy(&self) -> PaneHandle {
        PaneHandle::new(self.id, self.comp_tx.clone())
    }

    pub fn run(self, handler: impl FnMut(&PaneHandle, PaneEvent) -> Result<bool>) -> Result<()> {
        let Pane { id, receiver, filters, comp_tx, pane_count, .. } = self;
        let proxy = PaneHandle::new(id, comp_tx.clone());

        let result = looper::run_closure(id, receiver, filters, proxy, handler);

        let _ = comp_tx.send(ClientToComp::RequestClose { pane: id });
        pane_count.fetch_sub(1, Ordering::Relaxed);

        result
    }

    /// Run the event loop with a Handler trait implementation.
    ///
    /// The weather widget pattern:
    /// ```ignore
    /// pane.run_with(Weather { city: "SF".into(), data: None })
    /// ```
    pub fn run_with(self, handler: impl Handler) -> Result<()> {
        let Pane { id, receiver, filters, comp_tx, pane_count, .. } = self;
        let proxy = PaneHandle::new(id, comp_tx.clone());

        let result = looper::run_handler(id, receiver, filters, proxy, handler);

        let _ = comp_tx.send(ClientToComp::RequestClose { pane: id });
        pane_count.fetch_sub(1, Ordering::Relaxed);

        result
    }

    /// Update the pane's title.
    pub fn set_title(&self, title: PaneTitle) -> Result<()> {
        self.comp_tx.send(ClientToComp::SetTitle {
            pane: self.id,
            title,
        }).map_err(|_| PaneError::Disconnected)?;
        Ok(())
    }

    /// Update the pane's command vocabulary.
    pub fn set_vocabulary(&self, vocabulary: CommandVocabulary) -> Result<()> {
        self.comp_tx.send(ClientToComp::SetVocabulary {
            pane: self.id,
            vocabulary,
        }).map_err(|_| PaneError::Disconnected)?;
        Ok(())
    }

    /// Update the pane's body content.
    pub fn set_content(&self, content: &[u8]) -> Result<()> {
        self.comp_tx.send(ClientToComp::SetContent {
            pane: self.id,
            content: content.to_vec(),
        }).map_err(|_| PaneError::Disconnected)?;
        Ok(())
    }
}

