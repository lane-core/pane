# Pane Project Current State (2026-04-06, mid-session)

## Implementation status

Five crates, 191 tests (189 unit + 2 doc-tests):

- **pane-proto** (12 files, 86 tests) — Protocol vocabulary, no IO. Message, Protocol, ServiceId, Flow, Handler, Handles<P>, MessageFilter, MonadicLens<S,A>, obligation handles (ReplyPort, CompletionReplyPort, CancelHandle), PeerAuth + AuthSource, Address, ControlMessage (7 variants), ServiceFrame (4 variants).
- **pane-session** (6 files, 45 tests) — Session-typed IPC. Transport trait (Read+Write blanket), TransportSplit trait, MemoryTransport (with split()), bridge (FrameCodec from byte zero, connect_and_run with reader+writer threads, ClientConnection with write_tx), FrameCodec (wire framing, permissive mode for client reader and server), handshake (Hello with provides, Welcome, Rejection/RejectReason, ServiceProvision/ServiceBinding, par session types), peer_cred (SO_PEERCRED/getpeereid), ProtocolServer (single-threaded actor, sends Ready after Welcome, provider index, DeclareInterest routing, RevokeInterest cleanup, frame forwarding with session_id rewriting).
- **pane-app** (9 files, 43 tests + 10 integration) — Actor framework. Pane, PaneBuilder<H> (with connect(), serve<P>(), real open_service<P>() via DeclareInterest, run_with()), ServiceDispatch<H> (session_id → type-erased fn), Dispatch<H>, LooperCore<H> (channel-driven run(), dispatch_lifecycle, dispatch_service for Notification), Messenger (with Address), ServiceHandle<P> (send_request, send_notification, Drop sends RevokeInterest), ExitReason.
- **pane-fs** (3 files, 5 tests) — Filesystem namespace. AttrReader<S>, AttrSet<S>, AttrValue, PaneEntry<S>.
- **pane-hello** (1 file, 0 tests) — First running pane app. Stub server + client over unix socket.

Integration tests: open_service_via_protocol_server, open_service_declined, revoke_interest_end_to_end, ready_buffered_during_open_service, multiple_open_service_sequential, connection_drop_delivers_service_teardown, self_provide_interest_declined, two_pane_echo_roundtrip, notification_round_trip, plus 2 vertical slice tests in looper_core.

## Key architectural decisions this session

1. **Writer thread per connection** — deadlock-free by DAG topology (DLfActRiS Theorem 5.4). Chosen over Arc<Mutex<Writer>> after session-type consultant identified circular-wait scenario with full kernel write buffers.
2. **Single LooperMessage enum** — Control(ControlMessage) + Service { session_id, payload }. Single channel preserves causal ordering (BLooper one-port model).
3. **Permissive codec on client reader** — dynamically assigned session_ids from DeclareInterest accepted; validation at looper dispatch table. I12 invariant shifted from codec to looper.
4. **ServiceDispatch<H>** — labeled coproduct eliminator (routing table, NOT an optic). Type erasure at the table is the correct duploid move (session-type consultant).
5. **ProtocolServer sends Ready** — after Welcome, matching Be's B_READY_TO_RUN posted by BApplication constructor (Application.cpp:497).
6. **Lazy DeclareInterest** — PaneBuilder.open_service sends DeclareInterest during setup, blocks for response, buffers unrelated messages (including Ready). Eager Hello.interests deferred as optimization.

## Process changes this session

- **STYLEGUIDE.md** — new universal style guide at project root, consolidating serena style_and_conventions. All contributors (human and agent) follow it.
- **rustfmt.toml** — max_width=100, imports_granularity="Crate", group_imports="StdExternalCrate".
- **CLAUDE.md** — now names the four-agent workflow explicitly with "one task per dispatch" note.
- **workflow.md** — trimmed to process + build only; style content moved to STYLEGUIDE.md.

## What's next

See PLAN.md. Priority order:

**A (remaining wiring):**
- Eager Hello.interests optimization (four-agent recommendation)
- Provider-side Request dispatch (deliver typed message + ReplyPort)
- Consumer-side Reply/Failed routing via Dispatch<H> by token
- ActivePhase<T> as explicit shift operator

**B (developer surface):**
- `pane::connect(signature)` entry point
- `#[pane::protocol_handler]` macro
- Closure form `Pane::run(|msg| ...)`
- Update pane-hello to use ProtocolServer (remove stub server)

**C (namespace layer):**
- FUSE mount serving pane-fs state
- Blocking-read observer file

## Dev workflow

`cargo test --workspace` — 191 tests, all passing.
`cargo fmt` — via rustfmt.toml.
`cargo run -p pane-hello` — prints "Hello, world", exits gracefully.
