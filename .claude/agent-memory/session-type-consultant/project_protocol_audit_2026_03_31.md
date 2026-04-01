---
name: Full protocol audit 2026-03-31
description: Comprehensive concurrency/protocol audit — 8 reviewer findings vetted, 4 new findings, affine gap inventory, duality verified
type: project
---

Full audit of pane codebase against session-type theory. 2026-03-31.

**Critical findings:**
- Server-side pane ownership NOT verified in handle_message — any client can mutate any pane. Blocks distributed deployment.
- ReconnectingTransport replays outgoing bytes but server sees new connection — no session resumption, replayed active-phase bytes hit handshake parser. Not yet integrated into App::connect_remote (safe for now).
- Chan<S,T> has no Drop impl — silent drop at non-End state leaves peer blocked. OK for bounded handshake, would need Drop impl if session types expand to active phase.
- Message::Clone is partial function — panics on 4 variants carrying typestate handles. send_periodic exposes this; send_periodic_fn is the correct replacement.

**Confirmed bugs:**
- UndoManager::edit_count wrapping_sub wraps to usize::MAX on underflow, permanently breaks is_saved().
- CloseAck silently dropped by catch-all _ => None in try_from_comp. Anti-pattern: hides future variants.

**Verified correct:**
- TimerToken dual-path cancel-on-drop: AtomicBool (Release/Acquire) + CancelTimer message. Idempotent.
- HasDual impls match standard duality. ServerHandshake is correct dual of ClientHandshake.
- ReplyPort, CompletionReplyPort, ClipboardWriteLock, PaneCreateFuture: all have correct Drop compensation.
- Handshake protocol: server correctly follows dual, Select/Branch correct.

**Affine gap inventory:**
- ReplyPort: Drop sends ReplyFailed (correct)
- CompletionReplyPort: Drop sends empty completions (correct)
- ClipboardWriteLock: Drop sends Revert (correct)
- PaneCreateFuture: Drop spawns cleanup thread with RequestClose (correct, bounded 10s)
- TimerToken: Drop calls cancel() (correct)
- Chan<S,T>: Drop is silent (gap — compensated by SessionError::Disconnected on peer)
- FilterToken: No Drop impl (minor — filter stays active, no protocol violation)

**Why:** Establishes baseline protocol health before distributed deployment and Tier 2 features.
**How to apply:** Reference these findings when reviewing changes to protocol.rs, event.rs, reconnecting.rs, or pane-server lib.rs. The ownership check (Finding 7) is a prerequisite for headless/distributed.
