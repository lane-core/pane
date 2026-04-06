# Formal-Verifier Session Report (2026-04-06)

## Scope

Audit of service registration wiring implementation: commits 7cfe35f..141ab36 (6 commits by pane-architect). 185 tests pass, up from 173.

---

## 1. Invariant Status Table

| Invariant | Previous (04-05) | Current (04-06) | Evidence | Notes |
|-----------|-------------------|------------------|----------|-------|
| **I1** (panic=unwind, Drop fires) | partial | partial | Unchanged — obligation handle unwind tests still pass | No regression |
| **I2** (no blocking in handlers) | untestable | untestable | No change | Needs timeout watchdog |
| **I3** (handlers terminate) | untestable | untestable | No change | Needs timeout watchdog |
| **I4** (typestate handles) | tested | tested | No regression | |
| **I5** (filters see only Clone-safe Messages) | partial | partial | No change | Filter bypass path untested |
| **I6** (sequential single-thread dispatch) | partial | **ADVANCED** | `LooperCore.run()` dispatches both `LooperMessage::Control` and `LooperMessage::Service` sequentially on one thread via `mpsc::Receiver` blocking recv. Service dispatch goes through same sequential path as lifecycle. | Still needs calloop for full I6 |
| **I7** (service dispatch fn pointers sequential) | untested | **TESTED** | `ServiceDispatch<H>::dispatch_notification` called from `LooperCore::dispatch_service`, which is called from `run()` on the single looper thread. `run_dispatches_service_notification` test validates end-to-end. | fn-pointer table now exists and is exercised |
| **I8** (send_and_wait panics from looper thread) | N/A | N/A | send_and_wait not implemented | |
| **I9** (dispatch cleared before handler drop) | tested | tested | No regression | |
| **I10** (ProtocolAbort non-blocking) | partial | partial | No change | |
| **I11** (ProtocolAbort at framing layer) | tested | tested | No change | |
| **I12** (unknown discriminant → connection error) | tested | **SHIFTED** | Client reader now uses `FrameCodec::permissive()` — accepts ALL service discriminants. Validation moves to the looper: `dispatch_service` returns `DispatchOutcome::Continue` for unknown session_ids (soft drop, not connection error). Server-side codec is also permissive. The I12 invariant as stated in the architecture spec ("unknown service discriminant → connection-level error") is NOT enforced on the client side. | See Structural Issues section |
| **I13** (open_service blocks until accepted) | partial | **TESTED end-to-end** | `builder_integration::open_service_via_protocol_server` — PaneBuilder.connect() + open_service() sends DeclareInterest through real ProtocolServer, blocks on mpsc recv for InterestAccepted, returns ServiceHandle. `open_service_declined_returns_none` tests the decline path. | Full end-to-end coverage |
| **S1** (token uniqueness) | tested | tested | No regression | |
| **S2** (sequential dispatch) | follows from I6 | **ADVANCED** | Service frames dispatched in same sequential loop as lifecycle. `run()` handles `LooperMessage::Service` arm inline with Control arm — no interleaving possible. | Follows from I6 advancement |
| **S3** (control-before-events in batch) | N/A | N/A | No batch processing yet | |
| **S4** (fail_connection scoped) | tested | tested | No regression | |
| **S5** (cancel without callbacks) | tested | tested | No regression | |
| **S6** (panic=unwind) | follows from I1 | follows from I1 | No regression | |

### Summary of movements
- **I6**: partial → advanced (service + lifecycle both sequential)
- **I7**: untested → tested (ServiceDispatch table exists and is exercised)
- **I12**: tested → shifted (permissive codec on client, soft-drop at looper)
- **I13**: partial → tested end-to-end
- **S2**: follows from I6 → advanced (same sequential loop)

---

## 2. Deadlock Analysis

### Verdict: SOUND

### DAG topology

```
PaneBuilder::open_service()
  → write_tx.send() [unbounded mpsc, non-blocking for sender]
    → writer thread recv() → codec.write_frame() → transport write
      → wire
        → server reader thread → event_tx.send() [unbounded mpsc]
          → server actor thread: process_control()
            → write_handle.write_frame() [leaf mutex, one per connection]
              → transport write → wire
                → client reader thread: codec.read_frame() [blocking on transport]
                  → msg_tx.send() [unbounded mpsc]
                    → PaneBuilder::open_service() rx.recv() [blocking]
```

### Analysis

1. **All intermediate sends are non-blocking.** Both `write_tx` and `msg_tx` are unbounded `mpsc::channel()`. The `mpsc::Sender::send()` on unbounded channels never blocks the sender.

2. **No directed cycle.** The wait-for relation is:
   - open_service blocks on `rx.recv()` — waits for reader thread
   - reader thread blocks on `codec.read_frame()` — waits for transport (server response)
   - server actor blocks on `event_rx.recv()` — waits for server reader threads
   - server reader thread blocks on `codec.read_frame()` — waits for transport (client message via writer thread)
   - writer thread blocks on `write_rx.recv()` — waits for open_service's `write_tx.send()`
   
   The chain is: open_service → (send to writer, non-blocking) → writer thread → wire → server → wire → reader thread → (send to open_service, non-blocking). No node waits for itself.

3. **Setup-phase blocking is safe.** `open_service` blocks on `rx.recv()`, but the response comes from the server via the reader thread. The writer thread is independent — open_service sends to `write_tx` (non-blocking), then blocks on `rx` (different channel). No cycle.

4. **Server write handles are leaf mutexes.** `WriteHandle.writer` is `Arc<Mutex<Box<dyn Write>>>`. Only the actor thread writes. The mutex is never held while acquiring another lock. Per the comment: "Leaf lock: never held while acquiring routing state."

5. **DLfActRiS correspondence.** The single-mailbox actor topology (one `event_rx` channel, sequential processing) matches DLfActRiS Theorem 5.4 conditions. The writer thread is a separate actor with its own single mailbox (`write_rx`).

### Residual risk

The `WriteHandle` mutex is unnecessary in the current topology (only the actor thread writes), but is not harmful — it's a leaf lock. If future code acquires it from a non-actor thread, the leaf property must be maintained.

---

## 3. Polarity Audit

### Verdict: CLEAN — no violations found

**ServiceFrame variants are all positive.** Confirmed in `service_frame.rs` doc comment (line 21): "all four variants are positive — they are serialized values on the wire." Code matches: `Request { token, payload: Vec<u8> }`, `Reply { token, payload: Vec<u8> }`, `Failed { token }`, `Notification { payload: Vec<u8> }` — all contain only serializable data.

**Handles<P>::receive is negative.** It's a callback awaiting invocation (`&mut self` → `Flow`). Confirmed in trait definition.

**ServiceDispatch is positive structure holding negative values.** `service_dispatch.rs` line 17-20 documents this explicitly: "The table is a positive structure holding negative values (closures awaiting invocation). Type erasure at this boundary is the correct duploid move." Code: `HashMap<u8, ServiceReceiver<H>>` where `ServiceReceiver<H> = Box<dyn Fn(&mut H, &Messenger, &[u8]) -> Flow>`. HashMap is positive; closures are negative.

**Exactly one polarity crossing per dispatch.** The path is:
1. Wire frame arrives (positive: bytes)
2. `reader_loop` wraps in `LooperMessage::Service { session_id, payload }` (positive)
3. `LooperCore::run()` calls `dispatch_service(session_id, &payload)` (still positive — data flowing)
4. `dispatch_service` parses `ServiceFrame` (positive → positive, no crossing)
5. For `Notification`: calls `service_dispatch.dispatch_notification()` which calls `receiver(handler, messenger, payload)` — **this is the single polarity crossing**: positive payload → negative closure invocation
6. Inside the closure: `postcard::from_bytes::<P::Message>(payload)` (↓P deserialize) then `handler.receive(msg)` (negative dispatch)

One crossing. Correct.

**Setup phase is all-positive.** `PaneBuilder::connect()` sends `Hello` (positive), receives `Welcome` (positive). `open_service` sends `DeclareInterest` (positive), receives `InterestAccepted` (positive), constructs `ServiceHandle` (positive data structure) and registers closure in `ServiceDispatch` (positive structure). The closures are constructed but not invoked during setup — no polarity crossing until `run_with`.

---

## 4. Test Gap Analysis

### Previous high-priority gaps (from 04-05)

| Gap | Status | Evidence |
|-----|--------|----------|
| 1. Connection drop → ServiceTeardown delivery to peer | **STILL UNTESTED** | `server_state_remove_connection_cleans_up` tests state cleanup and returns `peers_to_notify`, but no test verifies the ServiceTeardown message actually arrives at the peer's reader. The server code synthesizes it (server.rs:315), but delivery is untested end-to-end. |
| 2. Self-provide rejection | **COVERED but misleading** | `handle_declare_interest` (server.rs:153) returns `None` when `provider_conn == consumer_conn`, which produces `InterestDeclined { reason: ServiceUnknown }`. The rejection works, but the reason is wrong — `ServiceUnknown` when the service IS known (just self-provided). No dedicated test for this path. |
| 3. Session_id overflow at 255 | **STILL UNTESTED** | `alloc_session` (server.rs:130) has `assert!(session < 255)`, but no test exercises this boundary. The assert will panic the server actor thread on overflow — a hard crash, not a graceful decline. |
| 4. ServiceHandle Drop → RevokeInterest | **TESTED** | `service_handle::tests::drop_sends_revoke_interest` verifies the Drop impl serializes `ControlMessage::RevokeInterest` to the write channel with correct session_id. `server_state_revoke_interest_cleans_route` verifies server-side route cleanup. |

### New test gaps introduced by wiring

| Gap | Severity | Description |
|-----|----------|-------------|
| **N1** | High | **RevokeInterest end-to-end** — ServiceHandle Drop sends RevokeInterest via write_tx, but no integration test verifies the message traverses writer thread → wire → server actor → route cleanup → ServiceTeardown to provider. The unit tests verify each hop independently but not the full path. |
| **N2** | Medium | **Buffered message drain in run_with** — `PaneBuilder::run_with` drains `buffered_messages` before entering the main loop. No test verifies that messages buffered during `open_service` (e.g., a lifecycle Ready that arrived while waiting for InterestAccepted) are correctly replayed. |
| **N3** | Medium | **Multiple open_service calls** — No test exercises two sequential `open_service` calls with different protocols. The `registered_services` HashSet prevents duplicates, but the interaction of two DeclareInterest/InterestAccepted exchanges (and potential message interleaving) is untested. |
| **N4** | Low | **Writer thread shutdown** — When `write_tx` is dropped (PaneBuilder/ServiceHandle cleanup), the writer thread should exit cleanly. No test verifies this teardown path. |
| **N5** | Low | **Malformed ServiceFrame in dispatch_service** — `dispatch_service` returns `DispatchOutcome::Continue` on deserialization failure (line 253). No test exercises this path. |

---

## 5. Doc Drift Report

### architecture.md

| Line | Issue | Suggested fix |
|------|-------|---------------|
| 182 | `looper_tx: LooperSender` — Pane struct shows field that doesn't exist in impl. Impl has no looper_tx; bridge uses mpsc channels directly. | Spec describes full architecture; impl is Phase 1. **No fix needed** — spec is ahead of impl by design. |
| 213 | `dispatch_table: Vec<ServiceDispatchEntry>` — PaneBuilder shows `Vec<ServiceDispatchEntry>`. Impl uses `ServiceDispatch<H>` (HashMap-based). | Same — spec is aspirational. Naming divergence is notable: spec says `ServiceDispatchEntry`, impl says `ServiceReceiver<H>`. |
| 322-328 | ServiceHandle Drop sends `LooperMessage::RevokeInterest` through `looper_tx`. Impl sends `ControlMessage::RevokeInterest` through `write_tx` (directly to wire). | **Architectural divergence.** The spec routes through the looper; the impl routes directly to the wire, bypassing the looper. This is correct for Phase 1 (no looper mediation needed), but the spec should acknowledge the direct-write path. |
| 408 | `ServiceTeardown { service: u8, reason: TeardownReason }` — field name is `service`. Impl (control.rs:55) uses `session_id: u8`. | **Fix:** change `service` to `session_id` in the spec. |

### .serena/memories/pane/current_state.md

| Line | Issue | Suggested fix |
|------|-------|---------------|
| 5 | "Five crates, 173 tests" | **Fix:** update to "185 tests (183 unit + 2 doc-tests)" |
| 54 | "`cargo test --workspace` — 173 tests, all passing" | **Fix:** update to 185 |
| Various | Missing mentions of: `TransportSplit`, `LooperMessage`, `ClientConnection`, `ServiceDispatch<H>`, `writer_loop`, service frame dispatch in LooperCore, `PaneBuilder::connect()` | **Fix:** update the pane-session and pane-app descriptions to reflect new types |

### docs/superpowers/plans/2026-04-06-service-registration-wiring.md

| Line | Issue | Suggested fix |
|------|-------|---------------|
| 89, 94, 122 | References to `ClientReader` (old name) | **No fix needed** — this is a plan document showing the before/after. The rename to `ClientConnection` is documented at line 356. |
| 181 | "verify all 173 pass" | **No fix needed** — historical reference in plan. |

---

## 6. Structural Issues

### Resolved

| Issue | Resolution |
|-------|------------|
| **ServiceHandle Drop → RevokeInterest** (was TODO) | Implemented. Drop sends `ControlMessage::RevokeInterest` through `write_tx`. Server handles it in `process_control` (server.rs:268-289). Route cleanup + ServiceTeardown to peer. |

### Remaining

| Issue | Severity | Details |
|-------|----------|---------|
| **Dual ConnectionId** | Medium | `ConnectionId(pub u64)` defined in both `pane-session/server.rs:42` and `pane-app/dispatch.rs:16`. These are separate types — no shared import. Currently they don't interact (server uses its own, dispatch uses its own). When `Dispatch<H>` needs to correlate with server-assigned connection IDs, this must be reconciled. Phase 2 work. |
| **Token allocation divergence** | Medium | Architecture spec says tokens are per-Connection (AtomicU64, line 1335). Implementation has a GLOBAL `static NEXT_TOKEN: AtomicU64` in `service_handle.rs:25` and a per-Dispatch counter in `dispatch.rs:34`. These are separate namespaces: NEXT_TOKEN is for ServiceFrame wire tokens, Dispatch's counter is for internal callback correlation. The spec's "per-Connection" doesn't match either. Needs clarification when request/reply dispatch lands. |
| **I12 shift undocumented** | Medium | Client reader uses `FrameCodec::permissive()` (bridge.rs:322) — accepts ALL service discriminants without validation. The architecture spec (line 950) states "unknown service discriminant → connection-level error." The actual behavior is soft-drop at the looper (dispatch_service returns Continue for unknown session_ids). This is arguably better (more resilient), but the spec doesn't reflect it. The shift from codec-level to looper-level validation should be documented. |
| **Self-provide returns wrong DeclineReason** | Low | `handle_declare_interest` (server.rs:153) returns `None` for self-provide, which maps to `InterestDeclined { reason: ServiceUnknown }`. A dedicated `DeclineReason::SelfProvide` would be more informative. |
| **alloc_session panics on overflow** | Low | `alloc_session` (server.rs:130) uses `assert!(session < 255)` — panics the server actor thread if a connection exhausts 254 sessions. Should return an error and send InterestDeclined instead. Currently no test for this boundary. |

### New

| Issue | Severity | Details |
|-------|----------|---------|
| **PaneBuilder Drop is a no-op** | Low | `PaneBuilder::Drop` (builder.rs:234-238) has a comment "Revoke all accepted interests. Idempotent with ServiceHandle<P> Drop." but the body is empty. If a PaneBuilder is dropped without calling `run_with` (e.g., setup failure), accepted interests are NOT revoked unless the ServiceHandles are also dropped. The write_tx drop will eventually cause the writer thread to exit and the server to detect disconnect, but there's no explicit cleanup. |
| **run_with discards _exit_rx** | Low | `PaneBuilder::run_with` (builder.rs:199) creates `(exit_tx, _exit_rx)` but discards the receiver. The `LooperCore` sends exit reasons to `exit_tx`, but nobody reads them. In vertical slice tests, the exit_rx is checked. In the real run_with, the exit reason is returned directly from `core.run()`. The unused channel is harmless but the pattern differs from tests. |

---

## Overall Assessment

The service registration wiring is **structurally sound**. The deadlock analysis is clean, polarity discipline is maintained, and the key invariant I13 now has end-to-end test coverage. The biggest concern is the I12 shift (permissive codec on client) which changes the invariant's meaning without documentation. The dual ConnectionId and token allocation divergence are Phase 2 reconciliation tasks, not current bugs.

Test coverage advanced materially: 173 → 185 tests, with the critical `open_service` end-to-end path now covered. The remaining test gaps (N1-N5) are real but none represent correctness risks in the current topology — they become important when multi-connection or error-recovery paths are exercised.
