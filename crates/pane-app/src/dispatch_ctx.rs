//! Scoped dispatch context for request/reply entry installation.
//!
//! `DispatchCtx<H>` wraps `&mut Dispatch<H>` + `PeerScope`. It
//! exists only during handler dispatch — the lifetime is tied to
//! the LooperCore's split borrow. Cannot be stashed, cloned, or
//! sent.
//!
//! Design heritage: BeOS BHandler accessed its BLooper via
//! `this->Looper()` (`src/kits/app/Handler.cpp:287`, fLooper
//! field). Plan 9 devmnt handlers received `Mnt *m` context for
//! dispatch infrastructure access
//! (`reference/plan9/src/sys/src/9/port/devmnt.c:786-798`).
//! DispatchCtx is the Rust translation: a scoped, lifetime-bound
//! reference to the dispatch infrastructure, provided by the
//! framework during handler dispatch.
//!
//! The `H = Self` binding is enforced by the trait signature:
//! `HandlesRequest<P>::receive_request` takes
//! `&mut DispatchCtx<Self>`. Self IS the handler type. No cast,
//! no mismatch possible. This addresses the soundness concern:
//! `send_request<H>`'s generic H parameter is bound to Self at
//! the trait level, preventing the UB that would arise from
//! casting `*mut ()` to `&mut Dispatch<WrongType>` (Ferrite,
//! Chen/Balzer/Toninho ECOOP 2022, §4.2 — protocol fidelity
//! requires every send-receive pair is matched).

use crate::connection_source::SharedWriter;
use crate::dispatch::{Dispatch, DispatchEntry, PeerScope, Token};
use crate::reply_future::ReplyFuture;

/// Scoped dispatch context provided to request handlers.
///
/// Carries `&mut Dispatch<H>`, `PeerScope`, and an optional
/// `SharedWriter` for the active connection. The lifetime `'a`
/// ties the context to the LooperCore's dispatch cycle — it
/// cannot outlive the handler call.
///
/// D12 Part 2: when a SharedWriter is present, `send_request`
/// writes directly to the shared queue instead of going through
/// the mpsc channel. This eliminates the cross-thread primitive
/// overhead for looper-thread sends (90% of all sends).
pub struct DispatchCtx<'a, H> {
    dispatch: &'a mut Dispatch<H>,
    connection: PeerScope,
    /// Direct write path for looper-thread sends (D12).
    /// None when the context is created without a ConnectionSource
    /// (e.g., unit tests, or the pre-ConnectionSource code path).
    writer: Option<&'a SharedWriter>,
}

impl<'a, H> DispatchCtx<'a, H> {
    pub(crate) fn new(dispatch: &'a mut Dispatch<H>, connection: PeerScope) -> Self {
        DispatchCtx {
            dispatch,
            connection,
            writer: None,
        }
    }

    /// Create a DispatchCtx with a direct write path (D12).
    pub(crate) fn with_writer(
        dispatch: &'a mut Dispatch<H>,
        connection: PeerScope,
        writer: &'a SharedWriter,
    ) -> Self {
        DispatchCtx {
            dispatch,
            connection,
            writer: Some(writer),
        }
    }

    /// Enqueue a frame for writing via the direct path (D12).
    /// Returns true if the frame was enqueued directly, false if
    /// the caller should fall back to the mpsc channel.
    pub(crate) fn enqueue_frame(&self, service: u16, payload: &[u8]) -> bool {
        if let Some(writer) = self.writer {
            writer.enqueue(service, payload);
            true
        } else {
            false
        }
    }

    /// Install a dispatch entry (E-Suspend). Returns the Token
    /// for the wire frame.
    ///
    /// Design heritage: Plan 9 devmnt.c:786-790 linked Mntrpc
    /// onto m->queue under spinlock BEFORE the write at line 798.
    /// pane: DispatchEntry installed via ctx.insert() before wire
    /// send in send_request.
    pub(crate) fn insert(&mut self, entry: DispatchEntry<H>) -> Token {
        self.dispatch.insert(self.connection, entry)
    }

    /// Install an async dispatch entry (Phase 4 E-Suspend).
    /// Returns a Token (for the wire frame) and a ReplyFuture<T>
    /// to await on the calloop executor.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn insert_async<T: std::any::Any + Send>(
        &mut self,
        session_id: u16,
    ) -> (Token, ReplyFuture<T>) {
        self.dispatch.insert_async(self.connection, session_id)
    }

    /// The connection this dispatch context is scoped to.
    pub fn connection(&self) -> PeerScope {
        self.connection
    }

    /// Cancel an entry without firing callbacks. Used by
    /// try_send_request to roll back the DispatchEntry when the
    /// wire send fails (session-type-consultant requirement:
    /// orphaned entries must not persist).
    pub(crate) fn cancel(&mut self, token: Token) -> bool {
        self.dispatch.cancel(self.connection, token)
    }

    /// Whether sending another request would exceed the
    /// negotiated max_outstanding_requests cap (D9).
    pub(crate) fn would_exceed_cap(&self) -> bool {
        self.dispatch.would_exceed_cap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::Dispatch;
    use pane_proto::Flow;

    struct TestHandler;

    #[test]
    fn ctx_insert_returns_token_and_installs_entry() {
        let mut dispatch = Dispatch::new();
        let conn = PeerScope(1);
        let mut ctx = DispatchCtx::new(&mut dispatch, conn);

        let token = ctx.insert(DispatchEntry {
            session_id: 5,
            on_reply: Box::new(|_: &mut TestHandler, _, _| Flow::Continue),
            on_failed: Box::new(|_: &mut TestHandler, _| Flow::Continue),
        });

        assert_eq!(token, Token(0));
        assert_eq!(dispatch.len(), 1);
    }

    #[test]
    fn ctx_connection_returns_scope() {
        let mut dispatch = Dispatch::<TestHandler>::new();
        let conn = PeerScope(42);
        let ctx = DispatchCtx::new(&mut dispatch, conn);
        assert_eq!(ctx.connection(), PeerScope(42));
    }

    #[test]
    fn ctx_tokens_are_monotonic() {
        let mut dispatch = Dispatch::new();
        let conn = PeerScope(1);
        let mut ctx = DispatchCtx::new(&mut dispatch, conn);

        let t0 = ctx.insert(DispatchEntry {
            session_id: 0,
            on_reply: Box::new(|_: &mut TestHandler, _, _| Flow::Continue),
            on_failed: Box::new(|_: &mut TestHandler, _| Flow::Continue),
        });
        let t1 = ctx.insert(DispatchEntry {
            session_id: 0,
            on_reply: Box::new(|_: &mut TestHandler, _, _| Flow::Continue),
            on_failed: Box::new(|_: &mut TestHandler, _| Flow::Continue),
        });

        assert_eq!(t0, Token(0));
        assert_eq!(t1, Token(1));
    }

    #[test]
    fn ctx_insert_async_returns_token_and_future() {
        let mut dispatch = Dispatch::<TestHandler>::new();
        let conn = PeerScope(1);
        let mut ctx = DispatchCtx::new(&mut dispatch, conn);

        let (token, _future) = ctx.insert_async::<String>(5);
        assert_eq!(token, Token(0));
        assert_eq!(dispatch.len(), 1);

        // Clean up — clear drops ReplySender, which sends Disconnected
        dispatch.clear();
    }
}
