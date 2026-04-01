# Session Summary: 2026-03-31

20 commits. 178 tests passing. All review debt resolved.

## C1 Looper Evolution (Phase 1+2 complete)

**Phase 1:** Replaced `mpsc::Receiver` + `recv_timeout` with `calloop::EventLoop<LooperState>`. All senders use `calloop::channel::Sender`. Unbounded channel (bounded backpressure removed). `drain_channel` helper handles calloop's 1024-msg-per-dispatch limit. `coalesce_batch` factored out of the old `drain_and_coalesce`.

**Phase 2:** Replaced hand-rolled `Timers` struct with calloop `Timer` sources. Timer callbacks push directly into `LooperState.batch` via `&mut LooperState`. `TimerToken` is now non-Clone with cancel-on-drop (`Drop` impl calls `cancel()`). Dual-path cancellation: `AtomicBool` for immediate callback check + `CancelTimer` LooperMessage for eager source removal. Deleted ~100 lines (TimerEntry, Timers, next_timeout, fire_due).

## API Removals (no deprecations — pre-stable policy)

- **`send_periodic` removed** — `Message::Clone` is a partial function (panics on AppMessage, Reply, CompletionRequest, ClipboardLockGranted). `send_periodic_fn` is the only periodic API. Clone impl kept internally for ExitBroadcaster only.
- **`try_send_message` removed** — identical to `send_message` with unbounded channels.
- **`PaneError::ChannelFull` removed** — unreachable with unbounded calloop channel.

## Security/Correctness Fixes

- **Server pane ownership check** (pane-server): `handle_message` now verifies `client_id` owns the pane via `pane_owned_by()` before processing mutations. CreatePane exempt. 9P scoped fids per-connection; pane uses runtime equivalent.
- **Identity stored after handshake**: `ClientSession` now has `identity: Option<PeerIdentity>`. Threaded through `CompletedHandshake` enum in pane-headless.
- **TLS integration in pane-headless**: `--tls-cert`/`--tls-key` flags. rustls wraps TCP connections before session-typed handshake. ProxyTransport wraps TlsServerTransport for tracing.
- **pane_count TOCTOU race fixed** (pane.rs): early-exit path used `fetch_sub` + separate `load` — now checks `fetch_sub` return value.
- **Condvar lost notification fixed** (pane.rs): `notify_all` now acquires mutex lock first. Ordering upgraded to Release/Acquire.
- **`Instant::duration_since` panic fixed** (undo.rs): `saturating_duration_since`.
- **UndoManager `wrapping_sub` fixed**: `saturating_sub` prevents edit_count wrapping to usize::MAX.
- **`try_reconnect` recursion → iteration**: mutual recursion via `replay_buffer` eliminated.

## Protocol Improvements

- **Exhaustive `try_from_comp` match** (event.rs): all 13 `CompToClient` variants explicit — no `_ => None` catch-all. CloseAck, PaneCreated, PaneRefused explicitly return None with comments. Compiler catches new variants.
- **`send_and_wait` mutual deadlock documented**: thread-local guard catches self-deadlock but not A↔B mutual. Documented alongside BeOS heritage.
- **Closure handler request asymmetry documented** on `Pane::run`: ReplyPort dropped, requestor gets ReplyFailed.

## Plan 9 Transport Patterns (new crate features)

- **ProxyTransport** (pane-session): generic `ProxyTransport<T, W>` wraps any Transport, logs send/recv with timestamps and hex preview. `--protocol-trace <file>` flag on pane-headless.
- **ReconnectingTransport** (pane-session): exponential backoff reconnection with message buffering and replay. From Plan 9's `aan(8)`.

## Plan 9 Formalization

- **`pane/plan9_divergences`** serena memory — comprehensive tracker (event loop, distributed arch, namespace, session, identity, timers, clipboard, plumbing, observer, connection resilience, diagnostics, terminal arch).
- **`pane/plan9_reference_insights`** serena memory — per-subsystem design guidance with man page citations.
- **`reference/plan9/`** directory — vendored full Plan 9 Programmer's Manual (565 man pages) + paper troff sources. MIT, Plan 9 Foundation.
- **`# Plan 9` doc annotations** on 10+ types/modules: App, PaneCreateFuture, looper.rs, Messenger, ExitBroadcaster, pane-session crate, pane-headless, PeerIdentity, Locality, ReconnectingTransport.
- **Workflow + style guide updated**: `docs/workflow.md` and `docs/kit-documentation-style.md` now require `# Plan 9` alongside `# BeOS` annotations.

## Heritage Annotations Added

`# BeOS` on: Message enum, Resize/CloseRequested/Pulse variants, Clipboard struct, Pane::run/run_with.
`# Plan 9` on: Locality enum (snarf federation), try_reconnect (aan pattern), TLS integration (exportfs -e).

## Documentation

- UndoManager: all 12 public methods documented.
- UndoPolicy: can_undo, can_redo, begin_group, end_group documented.
- KeyCombo::new, LinearPolicy::new documented.
- 7 stale references fixed (mpsc → calloop, drain_and_coalesce → coalesce_batch, fire_due, ChannelFull, recv_timeout).
- `beapi_divergences` updated (channel model, TimerToken).

## Process

- **No deprecations policy** — pre-stable API, remove dead code outright. See `feedback_no_deprecations.md`.
- **`pane/current_state`** serena memory updated to reflect post-C1 codebase.
- **`pane/code_review_findings_2026_03_31`** — full seven-reviewer audit findings archived.
