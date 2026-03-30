# Maty Integration Plan

Concrete spec language and implementation roadmap derived from the multiparty session types analysis (`research-multiparty-session-types.md`).

---

## Part 1: Spec Language to Integrate

### 1a. Architecture S7 — Protocol Design: Session Type Enrichments

The following paragraphs replace or extend the current "Session types" subsection of S7. They should be inserted after the current paragraph beginning "Pane uses a custom session type implementation" and before "The transport bridge."

---

**Event actor validation.** Fowler and Hu's Maty (OOPSLA) proves that event-driven actors participating in multiple sessions through a single event loop are deadlock-free across sessions, provided handlers terminate. Pane's compositor main thread — a calloop event loop with per-client `SessionSource` registrations — is this model. Each connected client is a session. The calloop callback is Maty's handler. The compositor never blocks on a single client; it processes whichever session has a pending message and yields back to the event loop. This is not an analogy — it is the same architecture, discovered empirically by BeOS's app_server (one Desktop thread multiplexing N ServerWindows) and proven safe by Maty's metatheory (Theorems 2 and 5: progress and global progress, with and without failure). The calloop integration is the event actor model realized in Rust without the formal effect system.

What this validation covers: inter-session deadlock freedom. A compositor that services 50 client sessions cannot deadlock on session A while session B has a pending message, because the event loop multiplexes — no handler blocks the loop. What it does not cover: intra-session correctness, which is the province of the binary session types on each client channel.

**Why binary session types suffice despite multiparty interactions.** The compositor mediates N client sessions, but N is dynamic (clients connect and disconnect at runtime) and there is no meaningful global type spanning all client interactions. Maty handles the same pattern — the paper's ID server and chat server both service N clients — not through N-party type constructors but through repeated binary session registration on a single access point, with the actor's internal state mediating cross-session coordination. Pane follows this pattern exactly: each compositor-client relationship is a binary session; the compositor's shared state (layout tree, focus tracking, tag index) coordinates between them. Multiparty thinking informs the protocol design — cross-client coordination scenarios (split creation, focus handoff, drag between panes) should be specified as global types for documentation and design validation. But the implementation remains binary sessions mediated by the compositor actor, which is what Maty prescribes for the dynamic-N case.

**Active phase decomposition.** The pane protocol has a nondeterminism problem: during the active phase, either side can send at any time. A strict session type like `Send<Content, Recv<Resize, ...>>` forces alternation that doesn't match reality. The resolution follows Maty's chat server pattern (Section 5.2): decompose into phases with different typing strategies.

- **Structured phases** (handshake, negotiation, teardown) use binary session types: `Send`/`Recv`/`Select`/`Branch`/`End`. The conversation has a fixed structure — message ordering matters, choices branch the protocol, and the session type captures this precisely.
- **The active phase** uses typed message enums (`ClientToComp` / `CompToClient`) dispatched through event-driven handlers. Both sides send when they have something to send. The guarantee shifts from ordering (session types) to exhaustive handling (Rust's `match` over the enum). This is Maty's handler pattern: the actor receives a typed message and pattern-matches all variants.

This decomposition resolves the nondeterminism question that arises from attempting to type the full bidirectional active phase as a single binary session. Session types apply where they provide value (structural phases with fixed ordering); typed enums with exhaustive matching apply where session types fight the protocol's natural shape (the repeating bidirectional phase). The boundary is explicit: the session type's terminal state transitions the channel into the active phase, after which the transport carries tagged enum messages.

---

### 1b. Architecture S7 — New Subsection: Protocol Phasing

This should be a new subsection within S7, after "Async by default" and before "Crash handling." Title: **Protocol phasing**.

---

**Protocol phasing.** The pane protocol decomposes into three phases, each with a typing strategy matched to its structure.

**Phase 1: Handshake (session-typed).** Client sends `ClientHello`; compositor responds with `ServerHello`; client sends capabilities; compositor selects accept or reject. This is a strict sequence with a branching choice — the natural domain of session types. The session type for the client side:

```
Send<ClientHello, Recv<ServerHello, Send<Capabilities, Branch<Recv<Accepted, ActiveTransition>, Recv<Rejected, End>>>>>
```

The compositor's side is the dual. `Select`/`Branch` encode the accept/reject choice: the compositor selects, the client offers both branches. The type guarantees exhaustive handling of both outcomes.

**Phase 2: Active (typed enums, event-driven).** After handshake, both sides communicate via typed message enums on the same socket, demultiplexed by a direction tag (1-byte prefix: 0x00 = client-to-compositor, 0x01 = compositor-to-client). Each direction has its own enum:

- `ClientToComp`: `ContentUpdate`, `TagUpdate`, `RequestClose`, `ScriptingQuery`, ...
- `CompToClient`: `Resize`, `Focus`, `InputEvent`, `TagRoute`, `ScriptingResponse`, ...

Both sides send when ready. The compositor dispatches via calloop handler; the client dispatches via its looper thread. Rust's exhaustive `match` guarantees every message variant is handled. There is no session-type ordering constraint during this phase — the constraint is type safety of each individual message, not sequencing between messages.

The active phase enums are extensible via negotiated capabilities from Phase 1. A client that negotiated a capability (e.g., `CAP_DIRECT_SCANOUT`) may send enum variants that a basic client cannot. The handshake captures this: the `Accepted` payload includes the resolved capability set, and the active phase enum is the union — unknown variants from a future protocol version are handled by a catch-all that logs and ignores, preserving forward compatibility.

**Phase 3: Teardown (session-typed).** Graceful close or crash boundary. Graceful: the active phase enums include `RequestClose` (client-initiated) and `CloseAck` (compositor-confirmed). Upon receiving `CloseAck`, the client closes the socket. The compositor removes the pane's state and continues. Crash: the socket drops. The compositor's calloop source fires `SessionEvent::Disconnected`. Cleanup proceeds identically to graceful close minus the acknowledgment. This is Maty's affine session model: a session can be abandoned at any point, and the surviving party handles the cancellation through its failure path (`Err(SessionError::Disconnected)` in pane's case, the failure continuation in Maty's).

The three-phase model makes the typing boundary explicit. Session types govern the edges (handshake, teardown) where protocol structure is load-bearing. Typed enums govern the middle (active phase) where bidirectional freedom is load-bearing. Neither strategy is compromised by being forced into the other's domain.

---

### 1c. Foundations S3 — Session Type Enrichment

The following sentence should be added to the end of the paragraph beginning "Session types formalize exactly this discipline" in S3 (after the sentence ending "...the theoretical framework that lets the compiler verify it."):

---

Fowler and Hu's event actor model (Maty, OOPSLA) provides a third leg: formal proof that an event-loop-based actor servicing multiple sessions is deadlock-free across those sessions, validating the architectural pattern BeOS's app_server proved empirically — a single event-driven coordinator multiplexing N client sessions, each independently session-typed.

---

### 1d. Architecture Sources — Addendum

Add to the Sources section:

---

- **Fowler & Hu (OOPSLA)**: "Speak Now: Safe Actor Programming with Multiparty Session Types." Maty — the event actor model that proves deadlock freedom for event-loop-based actors participating in multiple sessions. Validates pane's calloop + per-client SessionSource architecture.

---

## Part 2: pane-session Phase 3 Implementation Roadmap

Based on the Maty analysis, the current crate state (reviewed 2026-03-22), and the architecture spec's protocol phasing design.

### 2.1 Select/Branch Type Constructors

**What:** Internal choice (`Select`) and external choice (`Branch`) — the `(+)` and `&` from session type theory. These are required for the handshake phase's accept/reject branching and for any protocol negotiation.

**Wire encoding:** 1-byte tag prefix. The selecting side sends a tag byte (0x00 for left, 0x01 for right) before continuing with the chosen branch's protocol. The offering side reads the tag and dispatches.

**Type definitions** (new entries in `types.rs`):

```rust
use std::marker::PhantomData;

/// Internal choice: this endpoint selects one of two continuations.
///
/// In linear logic: A (+) B (plus — the selector decides).
/// Wire format: sends a 1-byte tag (0x00 = left, 0x01 = right),
/// then continues as the selected branch.
pub struct Select<L, R>(PhantomData<(L, R)>);

/// External choice: this endpoint receives the peer's selection.
///
/// In linear logic: A & B (with — the offerer handles both).
/// Wire format: reads a 1-byte tag, dispatches to the corresponding branch.
pub struct Branch<L, R>(PhantomData<(L, R)>);

/// Result of receiving a branch selection.
pub enum BranchResult<L, R> {
    Left(L),
    Right(R),
}
```

**Chan impls:**

```rust
impl<L, R, T: Transport> Chan<Select<L, R>, T> {
    /// Select the left branch. Sends tag 0x00, advances to L.
    pub fn select_left(mut self) -> Result<Chan<L, T>, SessionError> {
        self.transport.send_raw(&[0x00])?;
        Ok(self.advance())
    }

    /// Select the right branch. Sends tag 0x01, advances to R.
    pub fn select_right(mut self) -> Result<Chan<R, T>, SessionError> {
        self.transport.send_raw(&[0x01])?;
        Ok(self.advance())
    }
}

impl<L, R, T: Transport> Chan<Branch<L, R>, T> {
    /// Receive the peer's branch selection.
    /// Returns Left or Right — must handle both (exhaustive match).
    pub fn offer(mut self) -> Result<BranchResult<Chan<L, T>, Chan<R, T>>, SessionError> {
        let tag = self.transport.recv_raw()?;
        match tag.as_slice() {
            [0x00] => Ok(BranchResult::Left(self.advance())),
            [0x01] => Ok(BranchResult::Right(self.advance())),
            _ => Err(SessionError::Codec(postcard::Error::DeserializeBadEncoding)),
        }
    }
}
```

**Duality** (additions to `dual.rs`):

```rust
impl<L: HasDual, R: HasDual> HasDual for Select<L, R> {
    type Dual = Branch<Dual<L>, Dual<R>>;
}

impl<L: HasDual, R: HasDual> HasDual for Branch<L, R> {
    type Dual = Select<Dual<L>, Dual<R>>;
}
```

**N-ary choice:** For protocols with more than two branches, nest: `Select<A, Select<B, Select<C, End>>>`. The wire format generalizes naturally — depth encodes in the tag sequence. A macro for flat N-ary `select!` / `offer!` can be added later if the nesting is ergonomically painful, but start without it.

**Tests:** Extend `session_smoke.rs` with:
- Select/Branch round-trip over memory transport
- Select/Branch over unix transport
- Duality verification: `Dual<Select<Send<A, End>, End>>` = `Branch<Recv<A, End>, End>`
- Error case: peer sends invalid tag byte
- Error case: peer disconnects before sending tag

### 2.2 Three-Phase Protocol Pattern

**What:** A documented API pattern and helper types for the handshake-active-teardown lifecycle. Not a framework — concrete types that pane-proto will use.

**Phase transition type:**

```rust
/// Marker type: the session-typed handshake is complete.
/// Consuming this value transitions to the active phase.
///
/// The active phase is NOT session-typed. It uses typed message enums
/// dispatched through event-driven handlers (client looper or compositor
/// calloop). The session type's job ends here.
pub struct ActiveTransition;
```

The handshake session type terminates at `Send<Ready, End>` or similar. After `close()`, the same socket is reused for the active phase's typed enum messages. The `ActiveTransition` type is a documentation/API marker — it doesn't carry protocol semantics, but it names the boundary explicitly so that protocol definitions read clearly.

**Active phase direction tag:**

```rust
/// Direction prefix for active-phase messages on a shared socket.
/// Client-to-compositor and compositor-to-client share one socket;
/// this 1-byte prefix demultiplexes.
#[repr(u8)]
pub enum Direction {
    ClientToComp = 0x00,
    CompToClient = 0x01,
}
```

This belongs in pane-proto, not pane-session. pane-session provides the session type primitives; pane-proto defines the specific pane protocol using those primitives.

**Example handshake type** (this is pane-proto's job, shown here for concreteness):

```rust
// Client-side handshake protocol
type PaneHandshake = Send<ClientHello,
    Recv<ServerHello,
        Send<Capabilities,
            Branch<
                Recv<Accepted, Send<Ready, End>>,  // accepted -> active phase
                Recv<Rejected, End>,                // rejected -> done
            >>>>;

// Compositor side is Dual<PaneHandshake> — automatic.
```

### 2.3 Transport Trait Changes

**No breaking changes needed.** The current `Transport` trait (`send_raw` / `recv_raw`) handles Select/Branch naturally — the 1-byte tag is just another `send_raw` / `recv_raw` call. The framing (length-prefix in unix transport, direct pass-through in memory transport) works for any payload size including a single byte.

**One addition:** A `try_recv_raw` method for non-blocking receive, needed by calloop integration when checking for active-phase messages.

```rust
pub trait Transport: Sized {
    fn send_raw(&mut self, data: &[u8]) -> Result<(), SessionError>;
    fn recv_raw(&mut self) -> Result<Vec<u8>, SessionError>;

    /// Non-blocking receive. Returns `Ok(None)` if no data is available.
    /// Default implementation returns `Ok(None)` (transports that don't
    /// support non-blocking can ignore this).
    fn try_recv_raw(&mut self) -> Result<Option<Vec<u8>>, SessionError> {
        Ok(None)
    }
}
```

The unix transport implements this using the socket's non-blocking mode. The memory transport implements it via `mpsc::Receiver::try_recv`. This is needed for the compositor's calloop integration where the event loop must poll without blocking.

### 2.4 Chan Type Changes

**One addition:** Expose transport access for the phase transition. After the session-typed handshake completes (reaching `End`), the caller needs the underlying transport to continue with active-phase enum messaging.

```rust
impl<T: Transport> Chan<End, T> {
    /// Close the session and return the underlying transport.
    ///
    /// Used for phase transitions: after a session-typed handshake,
    /// the transport is reused for the active phase's typed enum messages.
    pub fn close_and_take(self) -> T {
        // Safety of the mem::read: we're consuming self, and we want
        // the transport without running Chan's drop (which would close it).
        // ManuallyDrop or similar pattern needed here.
        self.transport
    }
}
```

This requires changing `Chan`'s internal representation to support extraction without double-drop. Two options:

**Option A: `ManuallyDrop<T>`.** Wrap the transport field in `ManuallyDrop`. `close()` calls `ManuallyDrop::drop()`. `close_and_take()` calls `ManuallyDrop::take()`. Zero runtime cost for send/recv — `ManuallyDrop` is repr(transparent). Downside: must implement `Drop` for `Chan` to ensure the transport is dropped if the channel is abandoned (e.g., thread panic). But `Chan` is `#[must_use]`, so abandonment is a warning.

**Option B: `Option<T>`.** Wrap in `Option`. All operations use `.as_mut().unwrap()`. `close()` calls `.take()` and drops. `close_and_take()` calls `.take()` and returns. Downside: every send/recv pays for the Option branch (optimizable but not guaranteed).

**Recommendation: Option A** (`ManuallyDrop`). The `#[must_use]` on `Chan` means abandonment triggers a compiler warning. The `Drop` impl handles the panic case. Zero cost on the hot path.

Additionally, `UnixTransport` needs an accessor:

```rust
impl UnixTransport {
    /// Extract the underlying stream for calloop registration.
    /// Consumes the transport.
    pub fn into_stream(self) -> UnixStream {
        self.stream
    }
}
```

### 2.5 Calloop SessionSource Evolution

**Current state:** `SessionSource` delivers raw `SessionEvent::Message(Vec<u8>)`. The compositor deserializes manually. This is correct for the active phase but wrong for the handshake phase.

**Phase 3 design:** The handshake runs on a per-pane thread using `Chan<PaneHandshake, UnixTransport>` — fully session-typed, blocking reads (the per-pane thread can block without affecting the compositor). After the handshake succeeds:

1. The per-pane thread calls `close_and_take()` to extract the `UnixTransport`.
2. It calls `into_stream()` to get the `UnixStream`.
3. The stream is re-registered with calloop as a `SessionSource` for the active phase.
4. Active-phase messages are delivered as `SessionEvent::Message(Vec<u8>)` and deserialized as `ClientToComp` enums in the calloop handler.
5. Outbound `CompToClient` messages are sent via `calloop::write_message()` on the same stream.

This matches the architecture spec's three-tier threading model: the per-pane thread handles the session-typed handshake; the calloop main thread handles the event-driven active phase. The calloop `SessionSource` is Maty's handler registration — once installed, it processes messages from that session through the event loop.

**No changes to `SessionSource` itself are needed for Phase 3.** The current implementation (length-prefixed message accumulation, non-blocking reads, disconnect detection) is correct for active-phase dispatch. The change is in how sessions arrive at calloop: they are born on per-pane threads (session-typed) and graduate to calloop (enum-typed) after handshake.

### 2.6 Task List

Ordered by dependency. Estimated effort in parentheses.

1. **Select/Branch types + duality + tests** (1-2 days)
   - Add `Select<L, R>`, `Branch<L, R>`, `BranchResult<L, R>` to `types.rs`
   - Add duality impls to `dual.rs`
   - Export from `lib.rs`
   - Tests: memory transport round-trip, unix transport round-trip, invalid tag, disconnect-before-tag, duality compile-time check

2. **`close_and_take` + transport extraction** (0.5 day)
   - Refactor `Chan` to use `ManuallyDrop<T>` for the transport field
   - Implement `Drop` for `Chan` (drops transport if not already taken)
   - Add `Chan<End, T>::close_and_take() -> T`
   - Add `UnixTransport::into_stream() -> UnixStream`
   - Test: handshake then extract transport, verify socket is still live

3. **`try_recv_raw` on Transport trait** (0.5 day)
   - Add default impl returning `Ok(None)`
   - Implement for `UnixTransport` (set non-blocking, attempt read, restore)
   - Implement for `MemoryTransport` (`try_recv`)
   - Test: non-blocking recv returns None when empty, returns data when available

4. **Example handshake protocol** (1 day)
   - Define a minimal handshake in pane-proto (or in pane-session's test suite as proof of concept)
   - `ClientHello` / `ServerHello` / `Capabilities` / `Accepted` | `Rejected`
   - Full round-trip test: handshake over unix socket, branch on accept/reject, extract transport, send active-phase enum message on the same socket
   - This is the proof that the three-phase pattern works end-to-end

5. **Document the three-phase pattern** (0.5 day)
   - Rustdoc module-level documentation in pane-session explaining the pattern
   - Reference the Maty validation
   - Show the handshake-to-active transition with code examples

**Total estimated effort:** 3.5-5 days for a developer familiar with the codebase.

**Not in Phase 3 scope:**
- Recursive session types (Rec/Var) — the active phase uses typed enums, not recursive sessions
- Per-state failure continuations — defer until uniform `Err(Disconnected)` proves insufficient
- Global type specification language — a documentation concern, not a crate feature
- N-ary Select/Branch macro — add when nesting becomes painful, not before
