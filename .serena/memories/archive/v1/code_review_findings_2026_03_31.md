# Code Review Findings — 2026-03-31

Seven-reviewer audit (4 reviewers + 3 vetting agents: Be, Plan 9, session type).
Three concurrency bugs already fixed (pane_count race, Condvar lost notification, duration_since panic).

## Critical

### 1. Server pane ownership not verified (pane-server/src/lib.rs:153-234)
`handle_message` accepts any PaneId without checking `state.client_id == client_id`. A TCP client could modify/close another client's panes. Data for the check exists (`PaneState.client_id`, `ClientSession.panes`) but is never consulted.
- **Be**: kernel ports scoped tokens per-team. Runtime check needed.
- **Plan 9**: 9P fids scoped per-connection. `walk(5)`: "fid must be valid in the current session."
- **Session type**: maps to DLfActRiS resource ownership.
- **Fix**: Add `fn pane_owned_by(&self, client_id, pane) -> bool` and check at top of each match arm except CreatePane.

### 2. Identity discarded after handshake (pane-headless, pane-server)
`ServerHandshakeResult.identity` is logged then thrown away. `ClientSession` has no identity field. Even with ownership check, can't do identity-based access control.
- **Plan 9**: `Tattach` stored uname per-connection. Auth fid cryptographically bound identity.
- **Fix**: Add `identity: Option<PeerIdentity>` to `ClientSession`. Store on handshake completion.

### 3. Message::Clone panics on 4 variants (event.rs:116-150)
CompletionRequest, AppMessage, Reply, ClipboardLockGranted carry linear handles. Clone impl is a partial function — panics at runtime on these variants.
- **Session type**: unsound at type level. Cloning CompletionReplyPort creates two reply paths for one conversation.
- **Be**: BMessage was always copyable (flat data). pane's Message carries move-only handles — different design.
- **Fix**: Deprecate `send_periodic` (the only Clone consumer). Keep `send_periodic_fn` as primary API. Clone impl stays for internal coalescing (verified: coalescing uses move, not clone).

### 4. Chan has no Drop impl (pane-session/src/types.rs)
`Chan<S, T>` has `#[must_use]` but no Drop. Silent drop at non-End state leaves peer stuck on recv that never arrives.
- **Session type**: the most important affine gap. Compare to ReplyPort/PaneCreateFuture/ClipboardWriteLock which all have Drop compensation.
- OK for handshake (short-lived, I/O error catches it). Matters more when Chan is used for longer-lived protocols.

## Moderate

### 5. CloseAck catch-all `_ => None` (event.rs:164-189)
`try_from_comp` uses `_ => None` which silently drops CloseAck and hides future CompToClient variants from exhaustiveness checking.
- **Session type**: anti-pattern for typed protocols. Replace with explicit arms for all 13 variants.
- **Be**: Haiku handled AS_DELETE_WINDOW internally too, but with explicit code, not a catch-all.

### 6. Mutual send_and_wait deadlock (proxy.rs)
Thread-local `is_looper_thread()` prevents self-deadlock but not mutual deadlock (A waits on B while B waits on A). Same bug as BeOS.
- **Be**: BeBook warned against mutual synchronous SendMessage. Known limitation.
- **Fix**: Document. The async `send_request` path is the deadlock-free alternative. Timeout bounds the hang.

### 7. Closure handler silently fails requests (looper.rs:352-359)
`Pane::run` (closure form) drops ReplyPort on incoming requests — caller gets ReplyFailed with no explanation. Handler sees the message and may act on it thinking it succeeded.
- **Be**: every looper could receive and reply regardless of handler setup.
- **Fix**: Document prominently on `Pane::run`. Consider `tracing::warn!` when Request arrives on closure handler.

### 8. UndoManager edit_count wrapping_sub (undo.rs:212)
`wrapping_sub(1)` can wrap to usize::MAX if undo is called when edit_count is 0 (possible after clear()). Permanently breaks is_saved().
- **Fix**: Use `saturating_sub` or assert edit_count > 0.

### 9. ReconnectingTransport try_reconnect recursion (reconnecting.rs:213-249)
Mutual recursion: try_reconnect → replay_buffer → try_reconnect. Bounded by timeout but fragile.
- **Plan 9**: aan(8) was iterative.
- **Fix**: Convert to loop.

### 10. ReconnectingTransport destroys session state (reconnecting.rs)
Reconnection at framing layer preserves buffered bytes but not session-type state. If reconnection happens mid-protocol, the peers may be in different states.
- **Session type**: protocol violation if used during handshake. OK for active phase (free-typed).

## Design Debt (not blocking)

### 11. Filter chain lacks provenance (pane-app filter.rs)
`matches(&Message)` can't distinguish local vs remote origin. Matters for security filters on network-originated events.
- **Be**: BMessageFilter had B_REMOTE_SOURCE / B_LOCAL_SOURCE discrimination.

### 12. No FrameMoved / ScreenChanged equivalents
- **Be**: BWindow::FrameMoved, BWindow::ScreenChanged. Needed for multi-monitor/DPI awareness.
- Documented as deliberate gap. Add when compositor supports it.

### 13. Error info loss in App::connect (app.rs:106-110)
`format!("{:?}")` catch-all erases source chain. Fix: add specific match arm for Error::Session.

### 14. Active-phase tracing is receive-only (pane-headless state.rs)
Outbound CompToClient not traced. Plan 9's exportfs -d logged both directions.

### 15. MAX_MESSAGE_SIZE + framing function duplication (pane-proto/wire.rs vs pane-session/framing.rs)
Wrong dependency direction cited in comment. Real debt is the function duplication (vectored vs non-vectored write_framed).

### 16. TLS not integrated into pane-headless
TLS transport exists in pane-session but TCP listener is plaintext-only.

## Documentation Debt

### Missing `# BeOS` annotations (Be engineer drafted text for all):
- Message enum (event.rs:28)
- Resize, CloseRequested, Pulse variants
- Clipboard struct (clipboard.rs:29)
- Pane::run / run_with (pane.rs)

### Missing `# Plan 9` annotations:
- Locality enum (clipboard.rs) — snarf federation

### Missing doc comments:
- UndoPolicy::can_undo, can_redo
- UndoManager public methods (record, undo, redo, mark_saved, is_saved, etc.)
- KeyCombo::new, LinearPolicy::new
