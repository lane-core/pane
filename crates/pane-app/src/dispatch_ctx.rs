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
use std::future::Future;

/// Scoped dispatch context provided to request handlers.
///
/// Carries `&mut Dispatch<H>`, `PeerScope`, and optional
/// looper-thread resources (SharedWriter, Scheduler). The
/// lifetime `'a` ties the context to the LooperCore's dispatch
/// cycle — it cannot outlive the handler call.
///
/// D12 Part 2: when a SharedWriter is present, `send_request`
/// writes directly to the shared queue instead of going through
/// the mpsc channel. This eliminates the cross-thread primitive
/// overhead for looper-thread sends (90% of all sends).
///
/// Phase 5: when a Scheduler is present, `schedule()` spawns
/// async futures on the calloop executor. Futures run on the
/// looper thread during calloop's poll cycle (not during batch
/// dispatch), preserving S3 ordering and I6 single-thread
/// dispatch.
pub struct DispatchCtx<'a, H> {
    dispatch: &'a mut Dispatch<H>,
    connection: PeerScope,
    /// Direct write path for looper-thread sends (D12).
    /// None when the context is created without a ConnectionSource
    /// (e.g., unit tests, or the pre-ConnectionSource code path).
    writer: Option<&'a SharedWriter>,
    /// Scheduler for spawning async futures on the calloop executor.
    /// None when running without calloop (tests, non-calloop LooperCore::run).
    /// The future runs on the looper thread during calloop's poll cycle,
    /// not during batch dispatch — S3 ordering preserved.
    scheduler: Option<&'a calloop::futures::Scheduler<()>>,
}

impl<'a, H> DispatchCtx<'a, H> {
    pub(crate) fn new(dispatch: &'a mut Dispatch<H>, connection: PeerScope) -> Self {
        DispatchCtx {
            dispatch,
            connection,
            writer: None,
            scheduler: None,
        }
    }

    /// Create a DispatchCtx with a direct write path (D12) and
    /// optional async scheduler (Phase 5).
    pub(crate) fn with_writer(
        dispatch: &'a mut Dispatch<H>,
        connection: PeerScope,
        writer: Option<&'a SharedWriter>,
        scheduler: Option<&'a calloop::futures::Scheduler<()>>,
    ) -> Self {
        DispatchCtx {
            dispatch,
            connection,
            writer,
            scheduler,
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

    /// Spawn an async future on the calloop executor.
    ///
    /// The future runs on the looper thread during calloop's poll
    /// cycle — after the current batch dispatch completes, not
    /// during it (S3 ordering preserved). The handler (`&mut H`)
    /// cannot be captured by the future because it's borrowed, not
    /// `'static`. Futures communicate results via channels, shared
    /// state, or par's exchange pairs (see `insert_async` +
    /// `ReplyFuture`).
    ///
    /// Returns `true` if the future was scheduled, `false` if no
    /// scheduler is available (standalone/test mode). Callers that
    /// need async composition should check the return value or
    /// use `insert_async` which returns a `ReplyFuture` regardless.
    ///
    /// Design heritage: BeOS BLooper ran handler callbacks
    /// sequentially; async I/O was delegated to separate BLoopers.
    /// Plan 9's mountmux correlated sleeping readers, not inline
    /// continuations. `schedule` encodes the same separation:
    /// handlers own synchronous state mutations, futures compose
    /// external operations.
    pub fn schedule<F>(&self, future: F) -> bool
    where
        F: Future<Output = ()> + 'static,
    {
        if let Some(scheduler) = self.scheduler {
            // calloop Scheduler::schedule returns Result<(), ExecutorDestroyed>.
            // If the executor was destroyed (looper shutting down), the
            // future is silently dropped — same as a cancelled ReplyFuture.
            scheduler.schedule(future).is_ok()
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
    use std::cell::RefCell;
    use std::rc::Rc;

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

    // ── Claim: schedule returns false without a scheduler ────

    #[test]
    fn schedule_without_scheduler_returns_false() {
        let mut dispatch = Dispatch::<TestHandler>::new();
        let conn = PeerScope(1);
        let ctx = DispatchCtx::new(&mut dispatch, conn);

        let scheduled = ctx.schedule(async {});
        assert!(!scheduled, "schedule must return false without a scheduler");
    }

    // ── Claim: schedule spawns a future on the executor ─────

    #[test]
    fn schedule_spawns_future_on_executor() {
        let mut event_loop = calloop::EventLoop::<()>::try_new().unwrap();
        let handle = event_loop.handle();

        let (executor, scheduler) = calloop::futures::executor::<()>().unwrap();
        handle
            .insert_source(executor, |(), _, _: &mut ()| {})
            .unwrap();

        // Observable side effect — Rc is safe on the same thread.
        let result = Rc::new(RefCell::new(String::new()));
        let result_clone = result.clone();

        let mut dispatch = Dispatch::<TestHandler>::new();
        let conn = PeerScope(1);
        let ctx = DispatchCtx::with_writer(&mut dispatch, conn, None, Some(&scheduler));

        let scheduled = ctx.schedule(async move {
            *result_clone.borrow_mut() = "scheduled".to_string();
        });
        assert!(scheduled, "schedule must return true with a scheduler");

        // Future hasn't run yet — it runs during calloop dispatch.
        assert_eq!(*result.borrow(), "");

        // Drive the executor.
        let mut state = ();
        event_loop
            .dispatch(Some(std::time::Duration::ZERO), &mut state)
            .unwrap();

        assert_eq!(*result.borrow(), "scheduled");
    }

    // ── Claim: multiple schedule calls all execute ──────────

    #[test]
    fn schedule_multiple_futures() {
        let mut event_loop = calloop::EventLoop::<()>::try_new().unwrap();
        let handle = event_loop.handle();

        let (executor, scheduler) = calloop::futures::executor::<()>().unwrap();
        handle
            .insert_source(executor, |(), _, _: &mut ()| {})
            .unwrap();

        let counter = Rc::new(RefCell::new(0u32));

        let mut dispatch = Dispatch::<TestHandler>::new();
        let conn = PeerScope(1);
        let ctx = DispatchCtx::with_writer(&mut dispatch, conn, None, Some(&scheduler));

        for _ in 0..5 {
            let c = counter.clone();
            let ok = ctx.schedule(async move {
                *c.borrow_mut() += 1;
            });
            assert!(ok);
        }

        let mut state = ();
        event_loop
            .dispatch(Some(std::time::Duration::ZERO), &mut state)
            .unwrap();

        assert_eq!(*counter.borrow(), 5, "all five scheduled futures must run");
    }

    // ── Claim: schedule composes with insert_async / ReplyFuture ─

    #[test]
    fn schedule_composes_with_reply_future() {
        let mut event_loop = calloop::EventLoop::<()>::try_new().unwrap();
        let handle = event_loop.handle();

        let (executor, scheduler) = calloop::futures::executor::<()>().unwrap();
        handle
            .insert_source(executor, |(), _, _: &mut ()| {})
            .unwrap();

        let mut dispatch = Dispatch::<TestHandler>::new();
        let conn = PeerScope(1);
        let mut ctx = DispatchCtx::with_writer(&mut dispatch, conn, None, Some(&scheduler));

        // Phase 4 + Phase 5 composition: install async entry, get
        // ReplyFuture, schedule a future that awaits the reply.
        let (_token, reply_future) = ctx.insert_async::<String>(7);

        let result = Rc::new(RefCell::new(String::new()));
        let result_clone = result.clone();

        let ok = ctx.schedule(async move {
            match reply_future.recv().await {
                Ok(reply) => {
                    *result_clone.borrow_mut() = reply;
                }
                Err(_) => {
                    *result_clone.borrow_mut() = "error".to_string();
                }
            }
        });
        assert!(ok, "schedule must succeed");

        // Dispatch once — the future is waiting on the ReplyFuture,
        // which hasn't been resolved yet.
        let mut state = ();
        event_loop
            .dispatch(Some(std::time::Duration::ZERO), &mut state)
            .unwrap();
        assert_eq!(*result.borrow(), "", "future must be pending");

        // Resolve the reply via Dispatch::fire_reply — simulates
        // a wire reply arriving and being dispatched.
        let mut handler = TestHandler;
        let messenger = crate::Messenger::stub();
        dispatch.fire_reply(
            conn,
            Token(0),
            &mut handler,
            &messenger,
            Box::new("hello async".to_string()),
        );

        // Dispatch again — the ReplyFuture completes, the
        // scheduled future runs to completion.
        event_loop
            .dispatch(Some(std::time::Duration::ZERO), &mut state)
            .unwrap();

        assert_eq!(*result.borrow(), "hello async");
    }
}
