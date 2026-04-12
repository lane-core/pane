---
type: architecture
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [pane-session, session-types, par, IPC, ProtocolServer, framing, transport, bridge, invariants, stress-tests, watch, pane-death]
related: [decision/server_actor_model, architecture/looper, architecture/rustix_migration]
agents: [pane-architect, session-type-consultant, systems-engineer]
---

# pane-session Architecture Digest

## Summary

pane-session bridges par's session-typed in-process channels to IPC transports. It implements session-typed bidirectional messaging over byte streams (unix sockets, test transports) via serialization (postcard) and framing (length-prefixed service discriminants). The 51 unit tests verify codec, handshake, and server routing; 21 stress tests validate framing edge cases, concurrency, and invariant enforcement. ProtocolServer is a single-threaded actor with thin reader threads, enforcing sequential dispatch and preventing TOCTOU races in teardown. par governs the handshake (Send<Hello, Recv<Result<Welcome, Rejection>>>); the active phase is runtime-dispatched via FrameCodec service demux (not session-typed). Watch/Unwatch flow through the actor to populate a watch table; PaneExited fires on disconnect (one-shot, broadcast to all watchers). Peer credentials are extracted via rustix (Linux: SO_PEERCRED; macOS: getpeereid + LOCAL_PEERPID), with macOS still using FFI while Linux is migrated.

## Components

### Top-level modules

**lib.rs** (9 lines) — Re-exports par and public submodules; crate facade.

**frame.rs** (618 lines) — Length-prefixed framing codec. Format: `[length: u32 LE][service: u16 LE][payload: postcard bytes]`. FrameCodec tracks registered service discriminants (permissive mode for server, strict for clients). Service 0xFFFF reserved for ProtocolAbort (frame-level abort signal, not session-typed). Self-poisons on read error (AtomicBool, one-shot): all subsequent reads return Poisoned without touching the stream. Tests cover oversized frames, TooShort, UnknownService, Abort detection, boundary conditions (0xFFFE max valid discriminant), multi-frame sequencing (frame:70-79, 262-616).

**transport.rs** (309 lines) — Trait Transport (Read + Write + Send + 'static) and TransportSplit (splits into independent Reader/Writer for concurrent access). MemoryTransport: in-memory byte channel pair (mpsc-backed, EOF on peer drop). UnixStream::into_split via try_clone. ConnectError: Transport io::Error or Rejected(Rejection). Design heritage: Plan 9 mount(2) used single fd; pane splits explicitly for Rust ownership and safe concurrent threads (transport:11-16).

**handshake.rs** (108 lines) — par protocol types. ClientHandshake = Send<Hello, Recv<Result<Welcome, Rejection>>>; ServerHandshake = Dual<ClientHandshake>. Hello carries version, max_message_size, interests (Vec<ServiceInterest>), and provides (Vec<ServiceProvision> — services this pane offers). Welcome echoes version/max_message_size and binds services to session_ids. Rejection carries RejectReason (VersionMismatch, Unauthorized, ServerFull, ServiceUnavailable) and optional message. No choice/branching in session types: the Result is a value inside the single Send/Recv exchange (handshake:10-11).

**bridge.rs** (1053 lines) — Connects par's oneshot channels to IPC. Two phases: Phase 1 (verify_transport, returns Result) catches "server not running" before par. Phase 2 (handshake over verified transport) serializes par's Send/Recv via postcard + FrameCodec (on control channel, service 0). Spawns reader and writer threads after handshake succeeds. LooperMessage enum (Control | Service { session_id, payload }) unifies control and service frames on one mpsc channel, preserving causal ordering (bridge:38-55, heritage: BeOS BLooper one port, Plan 9 devmnt one fd). WriteMessage = (u16, Vec<u8>). WRITE_CHANNEL_CAPACITY = 128 (bounded backpressure; D9 in
`decision/connection_source_design` requires this to derive from
the negotiated `Welcome.max_outstanding_requests`, not be a
separate constant — pending bridge-side integration). HANDSHAKE_MAX_MESSAGE_SIZE = 16 MB. ClientConnection has welcome, rx (LooperMessage), write_tx (SyncSender<WriteMessage>); ServerReader is read-only (server actor owns writes). Reader threads are thin negative adapters (blocking reads on transport, post ServerEvents to actor channel). Writer thread drains write_tx, frames via FrameCodec, sends on transport. Codec set_max_message_size after handshake. FrameError during handshake panics (transport death is exceptional) (bridge:6-9). **Transition:** Bridge reader/writer threads are being replaced by ConnectionSource (calloop EventSource in pane-app). Handshake still runs in a short-lived bridge thread (D2, Option A); post-handshake fd handed off via `LooperMessage::NewConnection`. See `decision/connection_source_design` C1-C6.

**server.rs** (1167 lines) — ProtocolServer: single-threaded actor model. spawn ProtocolServer::new() launches actor thread immediately. accept(reader, writer) performs handshake on caller's thread, then posts ServerEvent::NewConnection to actor; spawns reader thread (reader_loop). ServerState: provider_index (ServiceId UUID → [ConnectionId]), routing_table ((ConnectionId, session_id) → Route), next_session (allocates u16, skips 0xFFFF), writers (WriteHandle per connection), conn_addresses (ConnectionId → Address), watch_table (watched ConnectionId → {watcher ConnectionIds}), watcher_reverse (watcher → {watched}). Reader threads read frames, post ServerEvent::{Frame | Disconnected} to mpsc channel. Actor thread exclusively owns ServerState, processes events sequentially (DLfActRiS star topology, Theorem 1.2 global progress). WriteHandle wraps Arc<Mutex<dyn Write>> and Arc<FrameCodec> — leaf lock, safe for sequential actor access. process_control routes DeclareInterest (allocates session_ids, wires consumer ↔ provider), RevokeInterest (tears down route, notifies peer via ServiceTeardown), Watch (resolves address to ConnectionId, adds to watch_table; unknown target → PaneExited immediately, no race), Unwatch (cleans both tables). process_service forwards frames via routing_table (missing route on Reply = Cancel/Reply race, silently dropped). process_disconnect fires PaneExited to all watchers (one-shot, EAct E-InvokeM semantics — watch entry consumed on delivery), cleans routing, providers, writers (server:416-475). actor_loop infinite loop: recv() ServerEvent, mutate ServerState, no Arc<Mutex> holding — pure sequential single-mail box. ConnectionHandle carries conn_id, hello, welcome, done_rx (signaled when reader thread exits).

**peer_cred.rs** (121 lines) — Extract PeerAuth from unix socket kernel credentials. Linux: rustix::net::sockopt::socket_peercred (already migrated). macOS: extern "C" getpeereid + getsockopt(SOL_LOCAL, LOCAL_PEERPID) — still hand-rolled FFI (peer_cred:65-98, marked for rustix migration in architecture/rustix_migration). Returns PeerAuth(uid, AuthSource::Kernel { pid }). Test: peer_cred_from_socket_pair verifies uid/pid match current process (peer_cred:106-119, uses rustix::process::getuid for verification only).

### Session Type API

pane-session does not expose a Chan<S, T> type or SessionEnum derive. par provides the session type primitives (Send, Recv, Dual, exchange types). pane-session wraps par at the handshake level only:

- **Handshake is session-typed:** ClientHandshake is par's native exchange protocol, par::exchange::Send/Recv drive the state machine.
- **Active phase is NOT session-typed:** After handshake, messaging is runtime-dispatched (service u16 demux, then looper-side routing by enum variant). No par types govern this. par intentionally stops; pane-app's looper and ServiceFrame enum take over.
- **No choice/branching in pane-session:** The handshake Result (Ok(Welcome) vs Err(Rejection)) is a value inside a single Send/Recv pair, not par's choose mechanism.
- **User perspective:** Handler receives a par session endpoint (ClientHandshake or ServerHandshake), calls send/recv directly for the handshake exchange. After handshake completes, handler leaves par; looper receives LooperMessages from bridge thread's mpsc channel (not par-typed).

### ProtocolServer: Single-Threaded Actor

Decision document: decision/server_actor_model (created 2026-04-05, enforced structurally).

**Architecture:**
- ProtocolServer::new() spawns one actor thread, returns handle with mpsc::Sender<ServerEvent>.
- Reader threads: spawned per accept(), decode frames, send ServerEvent::Frame to actor (zero shared state except mpsc endpoint).
- Actor thread: owns ServerState exclusively (no Arc<Mutex>), processes events sequentially in actor_loop.
- WriteHandle: Arc<Mutex<dyn Write>> — leaf lock, safe because only the actor writes via it.

**Single-threaded invariant enforced structurally:** ServerState is not wrapped in Arc<Mutex>; it lives on the actor thread's stack. Reader threads cannot mutate it; they can only send events. The actor's sequential loop is the only access path. No data race is possible.

**Theoretical basis:** EAct single-mailbox invariant (Fowler/Hu Definition 3.1). DLfActRiS star topology (actor at center, connections as leaves) is trivially acyclic — [JHK24] Theorem 1.2 proves ProtocolServer's **local** star topology is progress-safe. For whole-system progress, see [FH] EAct progress (Theorems 6 + 8) and `analysis/verification/invariants/inv_rw` (Inv-RW). Citation scoped per D3 in `decision/connection_source_design`.

### Invariants Enforced

**S1 (token uniqueness):** next_session per connection (alloc_session), starts at 1, skips 0xFFFF (reserved for ProtocolAbort). Test: stress tests exercise concurrent DeclareInterest (server:152-159). Enforced by single-threaded actor (no race on next_session HashMap).

**S2 (sequential dispatch):** Actor processes ServerEvents one at a time, no interleaving. Single-threaded, sequential loop (server:668-701).

**S3 (six-phase batch ordering):** Implemented in pane-app Looper, not pane-session. pane-session contributes by delivering LooperMessages in causal order (single mpsc channel, bridge:38-55).

**S4 (fail_connection scoped):** AcceptError bubbles from accept() to caller; transport failures during handshake are caught in Phase 1 (verify_transport) or explicitly handled in Phase 2 (bridge threads panic on mid-handshake failure, which aborts the session). Not a runtime invariant pane-session enforces post-handshake.

**S5 (cancel without callbacks):** Cancel control message handled by actor (server:349-350, TODO: forward to provider). Test coverage deferred.

**S6 (panic=unwind):** Handler's panic on session type abuse unwinds the handler and drops its par endpoint, terminating the session. bridge threads panic on malformed handshake frames (bridge:119, 145) or transport failure (abort connection). Tested implicitly.

**I10/I11 (ProtocolAbort):** Frame-level abort signal (service 0xFFFF). write_abort best-effort (does not block, propagates io::Error). read_frame detects and returns Frame::Abort. Tests: abort_write_format, abort_write_propagates_error, read_frame_detects_abort (frame:274-319). Invariant: 0xFFFF cannot be registered or written by application (panics on register_service/write_frame).

**I12 (unknown discriminant):** Unknown service → FrameError::UnknownService on strict codec, silent drop on permissive (server-side). Strict codec registers services explicitly; client codec only knows registered services. Test: unknown_service_is_connection_error (frame:338-349). Server uses permissive codec for dynamically allocated session_ids from DeclareInterest (server:557, bridge:359).

**I13 (open_service blocks):** Not directly enforced in pane-session. pane-app Looper enforces this via dispatch machinery and ServiceHandle blocking semantics.

**Poison on codec error:** After any FrameError, codec sets poisoned flag; all subsequent read_frame calls return Poisoned immediately. Prevents desync after Oversized/Transport errors (frame:180-189, stress test codec_desync_after_oversized_frame verifies).

### Watch/Unwatch, PaneExited (commit e5cd130)

**Mechanism:** ProtocolServer maintains watch_table (watched ConnectionId → {watcher ConnectionIds}) and watcher_reverse ({watchers} → {watched}). Both are owned by actor thread.

**Watch:** Consumer sends ControlMessage::Watch { target: Address }. Actor resolves Address to ConnectionId via linear scan in conn_addresses (server:173-178, O(n) Phase 1 acceptable; Phase 2 can add reverse index). If target found, add consumer to watch_table[target] and update reverse index. If target unknown (already disconnected or never connected), send PaneExited immediately to consumer (no race window, server:352-379).

**Unwatch:** Consumer sends ControlMessage::Unwatch { target }. Actor removes consumer from watch_table[target] and cleans both indices. Unknown target = no-op (server:381-396).

**PaneExited:** On disconnect, actor_loop calls process_disconnect. Actor iterates watch_table[disconnected_conn_id], sends ControlMessage::PaneExited { address, reason: Disconnected } to each watcher (one-shot delivery, fire-and-forget). Then cleans both watch tables (server:416-444). Design heritage: BeOS BRoster::StartWatching (registrar mediated watches, any-to-any).

**Invariant:** Watch entry is one-shot (EAct E-InvokeM semantics, Fowler/Hu S3.3): entry consumed on delivery, no re-triggering if watcher outlives watched pane.

### Stress Tests (21 total, marked #[ignore])

Tests validate framing, concurrency, and invariant enforcement under adversarial conditions:

1. **Codec resync (codec_desync_after_oversized_frame, oversized_frame_caller_stops_reading):** Verify poison flag prevents stream desync after Oversized/TooShort. Cursor stays at offset 4 (length prefix consumed, body not), poison blocks further reads.

2. **RevokeInterest/Request race (revoke_interest_request_race_reply_dropped):** Establish route, send Request, immediately RevokeInterest, provider replies. Server silently drops reply (no route). Stress test documents server behavior and consumer-side gap (request hangs, no failure signal).

3. **Concurrent accept (multiple threads call accept simultaneously):** Test handshake_state Mutex serializes conn_id allocation (no two connections get same id).

4. **Partial reads:** MemoryTransport.split() allows independent reader/writer threads; test sends data in chunks, reader pulls in smaller buffers, verifies coalescing.

5. **High concurrency DeclareInterest:** Multiple consumers request same service, session allocation (next_session) under load, routes wired correctly.

6. **Transport failure mid-frame:** EOF during length prefix, EOF during body, parser poisoned correctly.

7. **Service registration boundary:** service 0xFFFE (max valid) works, 0xFFFF (abort) panics.

8. **Payload containing 0xFFFF bytes:** Confirm abort detection is on service field only, not payload content.

9. **Multi-frame sequencing:** Two frames in one buffer, cursor advances correctly past each boundary.

Tests reuse ClientConn helper (manual handshake, control/service send/read) and accept_on_thread for server acceptance. Generic tests with proptest for frame randomization.

### Known Gaps

From status.md "What's next" Phase 1 (in progress):

**ConnectionSource** (C1-C6 landed in pane-app, not pane-session):
calloop EventSource wrapping a post-handshake UnixStream fd.
Non-blocking FrameReader/FrameWriter in pane-app's
`connection_source.rs`. Remaining: bridge-side integration —
replacing bridge reader/writer threads with ConnectionSource for
real connections. See `decision/connection_source_design`.

**Messenger wire send:** Watch/Unwatch stubs exist in pane-app; need write_tx on Messenger to send control frames to server (server:380, TODO).

**Cancel forwarding:** server:349-350, TODO: forward Cancel { token } to provider side (enables Tflush-like cancellation).

**DeclareInterest late-binding:** Currently returns static ServiceBinding in Welcome. Should dynamically allocate on each DeclareInterest request (requires active-phase session type for Request/Reply or moving to runtime dispatch in pane-app).

**TLS support:** Phase 2. pane-session currently accepts any Transport (Read + Write); stub for TLS stubs mentioned in Cargo description, not implemented.

**Agda formalization:** Four properties identified (decision/server_actor_model): ReplyPort exactly-once, Dispatch one-shot, destruction sequence ordering, install-before-wire. Deferred until architecture stabilizes.

### rustix Migration Status

**Per architecture/rustix_migration (created 2026-04-07):** Dependency already added to Cargo.toml (version 1.1.4, features net + process). Linux migration complete: peer_cred_linux uses rustix::net::sockopt::socket_peercred (line 54, via AsFd trait).

**macOS still FFI-direct:** peer_cred_macos (lines 65-98) uses hand-rolled extern "C" { getpeereid, getsockopt }, raw fd casts, unsafe blocks. rustix does not yet wrap getpeereid or LOCAL_PEERPID for macOS. Planned: wait for rustix 1.2+ or use nix crate for macOS-specific syscalls.

**Other modules:** transport.rs and server.rs (not yet built) will use rustix for socket syscalls when implemented. pane-proto and pane-app have no syscalls.

---

## See also

- **decision/server_actor_model** — Rationale for single-threaded actor architecture, EAct/DLfActRiS grounding
- **architecture/looper** — Six-phase batch ordering (S3), ControlMessage dispatch, LooperMessage receiver
- **architecture/rustix_migration** — FFI → rustix migration plan, current macOS blockers
- **status.md** — Test counts (51 + 21), invariant summary, Phase 1 priorities
- **Cargo.toml** — par 0.3.10 (session types), postcard 1 (framing), rustix 1.1.4 (peer creds), uuid 1 (service IDs)

**Files:**
- pane-session/src/lib.rs (9 L)
- pane-session/src/frame.rs (618 L) — FrameCodec, poison, tests
- pane-session/src/transport.rs (309 L) — Transport trait, MemoryTransport, split semantics
- pane-session/src/handshake.rs (108 L) — ClientHandshake, Welcome, Rejection
- pane-session/src/bridge.rs (1053 L) — Phase 1/2, par integration, reader/writer loops, LooperMessage
- pane-session/src/server.rs (1167 L) — ProtocolServer, ServerState, actor_loop, routing, watch table
- pane-session/src/peer_cred.rs (121 L) — PeerAuth extraction, rustix (Linux) + FFI (macOS)
- pane-session/tests/stress.rs (2390 L) — 21 stress tests, ClientConn helper, invariant validation
