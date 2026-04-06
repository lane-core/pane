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

use crate::dispatch::{Dispatch, DispatchEntry, PeerScope, Token};

/// Scoped dispatch context provided to request handlers.
///
/// Carries `&mut Dispatch<H>` and `PeerScope` for the active
/// connection. The lifetime `'a` ties the context to the
/// LooperCore's dispatch cycle — it cannot outlive the handler
/// call.
pub struct DispatchCtx<'a, H> {
    dispatch: &'a mut Dispatch<H>,
    connection: PeerScope,
}

impl<'a, H> DispatchCtx<'a, H> {
    pub(crate) fn new(dispatch: &'a mut Dispatch<H>, connection: PeerScope) -> Self {
        DispatchCtx {
            dispatch,
            connection,
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

    /// The connection this dispatch context is scoped to.
    pub fn connection(&self) -> PeerScope {
        self.connection
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
}
