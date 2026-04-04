# Structural Gaps: EAct Analysis of Pane's Actor Model

Four gaps identified by analyzing Fowler et al. EventActors against pane's architecture. Each gap includes what EAct reveals and how the architecture spec resolves it.

## Gap 1: Active Phase Sub-Protocols → Protocol Trait + DeclareInterest

**What EAct reveals:** The active phase contains sub-protocols with real structure (request-response, negotiation, correlated exchanges).

**Resolution (architecture spec):** The Protocol trait + DeclareInterest system gives each sub-protocol its own typed channel with per-service wire discriminants. Session types govern the handshake; per-protocol message types (Protocol::Message, requiring Serialize + DeserializeOwned) govern the active phase. Typestate handles (ReplyPort, ClipboardWriteLock, CancelHandle) enforce interaction patterns at the API surface (principle C2).

## Gap 2: Single-Session Looper → Multi-Source Dispatch

**What EAct reveals:** Actors must handle multiple heterogeneous sessions via the event loop.

**Resolution (architecture spec):** Each protocol relationship is a separate typed calloop source. The looper selects across all sources (ConnectionSource per Connection, plus per-service channels) and dispatches to Handler (lifecycle) or Handles<P> (Display, Clipboard, and all other protocols). Base Message is lifecycle only; display and service events dispatch through Handles<P>.

## Gap 3: Per-Conversation Failure → Dispatch<H>

**What EAct reveals:** EAct's `suspend` takes a failure callback per conversation. When a peer crashes mid-interaction, the specific conversation's failure handler fires.

**Resolution (architecture spec):** Dispatch<H> provides per-request typed callbacks. `send_request` registers (on_reply, on_failed) pairs. On Connection loss, `dispatch.fail_connection()` fires on_failed for every pending entry before `handler.disconnected()`. Per-conversation failure, not just actor-level death signals.

## Gap 4: Cascading Failure → Dispatch + ExitBroadcaster

**What EAct reveals:** EAct's "zapper threads" propagate failure through sessions — draining queued messages, invoking failure callbacks.

**Resolution (architecture spec):** When a pane exits: (1) Dispatch is cleared (fail_connection or clear) before handler drop, (2) ExitBroadcaster notifies watchers via pane_exited, (3) obligation handles fire Drop compensation (ReplyPort → ReplyFailed, ClipboardWriteLock → Revert). The invariant I9 ensures Dispatch is cleared before the handler is dropped. CancelHandle's inverted Drop (no-op) depends on this ordering.
