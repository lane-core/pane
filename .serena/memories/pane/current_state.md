# Pane Project Current State (2026-04-05, end of session)

## Implementation status

Five crates, 173 tests (171 unit + 2 doc-tests):

- **pane-proto** (12 files, 88 tests) — Protocol vocabulary, no IO. Message, Protocol, ServiceId, Flow, Handler, Handles<P>, MessageFilter, MonadicLens<S,A>, obligation handles (ReplyPort, CompletionReplyPort, CancelHandle), PeerAuth + AuthSource, Address, ControlMessage (7 variants), ServiceFrame (4 variants).
- **pane-session** (6 files, 40 tests) — Session-typed IPC. Transport trait (Read+Write blanket), MemoryTransport (with split()), bridge (FrameCodec from byte zero, connect_and_run/accept_and_run with reader loop), FrameCodec (wire framing, permissive mode for server), handshake (Hello with provides, Welcome, Rejection/RejectReason, ServiceProvision/ServiceBinding, par session types), peer_cred (SO_PEERCRED/getpeereid), ProtocolServer (single-threaded actor, provider index, DeclareInterest routing, frame forwarding with session_id rewriting).
- **pane-app** (8 files, 30 tests + 3 integration) — Actor framework. Pane, PaneBuilder<H> (with serve::<P>()), Dispatch<H>, LooperCore<H> (channel-driven run(), dispatch_lifecycle), Messenger (with Address), ServiceHandle<P> (send_request with real serialization, send_notification, with_channel), ExitReason.
- **pane-fs** (3 files, 5 tests) — Filesystem namespace. AttrReader<S>, AttrSet<S>, AttrValue, PaneEntry<S>.
- **pane-hello** (1 file, 0 tests) — First running pane app. Stub server + client over unix socket. Prints "Hello, world" and exits gracefully.

Integration tests: two_pane_echo_roundtrip (full Request/Reply through ProtocolServer), declare_interest_no_provider_declined, notification_round_trip, plus 2 vertical slice tests in looper_core.

## Key architectural decisions this session

1. **PeerAuth** — product-of-sum (uid always present, AuthSource is provenance). Four-agent design.
2. **Address** — lightweight copyable wire type. Direct pane-to-pane communication (not server-mediated only).
3. **send_request on ServiceHandle<P>** — protocol-scoped, not untyped on Messenger. Compile-time protocol agreement.
4. **Handshake rejection** — `Result<Welcome, Rejection>` in par session type. Explicit, not transport-close.
5. **FrameCodec from byte zero** — no raw handshake mode. Phase-aware deserialization on Control.
6. **Server as single-threaded actor** — prevents non-associative cross-polarity composition (duploid analysis). Reader threads are thin negative adapters.
7. **ServiceFrame** — untyped wire envelope. All variants positive. Polarity crossings at dispatch.

## Duploid theoretical framework

Active phase is a plain (non-dialogue) duploid with writer monad Ψ(A) = (A, Vec<Effect>) on positives and identity comonad on negatives. MonadicLens is a mixed optic (Clarke et al. Proposition 4.7). Sequential dispatch prevents non-associative composition. See serena `pane/duploid_analysis` and `pane/duploid_deep_analysis` for full analysis.

## What's next

See PLAN.md. Priority order:

**A (service registration wiring):**
- PaneBuilder::serve + open_service talking to real ProtocolServer (currently integration test does manual handshakes)
- ActivePhase<T> as explicit shift operator (ω_X from duploid analysis)

**B (developer surface):**
- `pane::connect(signature)` entry point
- `#[pane::protocol_handler]` macro for ergonomic dispatch
- Closure form `Pane::run(|msg| ...)`

**C (namespace layer):**
- FUSE mount serving pane-fs state
- Blocking-read observer file (`/pane/<n>/event`)

## Process improvements

Agent workflow updated (serena `pane/agent_workflow`): Steps 1-2 are a loop (iterate until design converges), follow-up rounds can use targeted agent subsets, formal-verifier produces grep-ready doc drift report, design agents note Rust-specific implications.

Heritage annotations with source citations are now standard (serena `style_and_conventions/heritage_annotations`). Every module has a Design heritage block citing specific Haiku source (file:line) and Plan 9 man pages.

## Dev workflow

`cargo test --workspace` — 173 tests, all passing.
`cargo run -p pane-hello` — prints "Hello, world", exits gracefully.
pane-hello connects over unix socket with PeerAuth verification.
