# pane Architecture

A pane is organized state with an interface for views of that state.
Display is one view. The namespace (`/pane/`, routed queries, remote
access) is another. Both are projections of the same state, kept
consistent by optic laws. The protocol governs how views are accessed,
negotiated, and coordinated.

pane is a protocol framework that happens to have a display mode. The
headless server is the base case. Display is a capability panes opt
into.

---

## Formal Foundation

Three published formalisms, one per layer:

```
par (CLL — Strba)            binary channel correctness
  ↓ adapted for IPC
pane-session                  transport, serialization, crash safety
  ↓ composed into actors
pane-app (EAct — Fowler et al.)   actor discipline over multiple sessions
```

**par** (Michal Strba, https://github.com/faiface/par) is a CLL
session type library for Rust. CLL (classical linear logic, Wadler
"Propositions as Sessions" JFP 2014) governs binary channel
correctness: Send/Recv duality, branching, streaming. par is
complete per its author. pane-session uses par as a direct
dependency — par's types (Send, Recv, etc.) are the phantom
state parameters on Chan<S, T>. pane-session provides the
Transport trait and postcard serialization for IPC. Chan
operations panic on disconnect (par's CLL model — sessions
complete or are annihilated; the looper's catch_unwind boundary
is the crash safety mechanism). The session types are par's
contribution; the IPC adaptation is pane-session's.

**EAct** (Fowler et al., "Safe Actor Programming with Multiparty
Session Types") governs how one actor interleaves work across
multiple sessions. pane-app IS the EAct framework:

| EAct concept | pane-app |
|---|---|
| Actor | Handler impl (one per pane) |
| Handler store σ | Handles\<P\> impls + Dispatch\<H\> entries |
| E-Suspend | `send_request` installs Dispatch entry |
| E-React | Reply arrives, entry consumed, callback fires |
| E-Reset | Handler returns Flow (back to idle — both Continue and Stop) |
| E-Raise | panic → catch_unwind (actor annihilated) |
| Progress | I6 + compliant protocols → system reduces |
| Global Progress | I2/I3/I8 + event-driven dispatch → eventual progress |

<!-- BeOS: BLooper = EAct actor. BHandler chain = σ. The formalism
proves why Be worked and reveals where it was unsound (dispatch
re-entrancy, untyped messages, no coverage checking). -->

**Optic discipline** (Clarke et al.) formalizes what Plan 9
achieved through filesystem convention. `/proc/N/status` (a
read-only lens onto process state), rio's per-window synthetic
files, per-process namespaces — optic projections. The optic
laws (GetPut, PutGet, PutPut) guarantee consistency between
views. Session types govern protocol relationships; optics
govern state-access relationships.

### Composition guarantee

CLL duality (HasDual) gives binary compliance — each binary
session's dual types match (Wadler JFP 2014, Theorem 1: cut
elimination = deadlock freedom for binary sessions). EAct's
Progress theorem (Thm 3.10) requires compliant protocols +
well-typed actors; CLL duality provides compliance. The single-
threaded execution model (I6) is pane's primary defense against
inter-session deadlocks, corresponding to EAct's event-driven
argument (§4.2.2, Progress). Global Progress requires handler termination
(I3) and no blocking in handlers (I2/I8).

Conditional protocol fidelity: if the developer's code type-checks
against `Chan<S, T>` and consumes every intermediate channel value,
the resulting protocol follows the structure of S. The consumption
condition is enforced by `#[must_use]` and Drop-based failure
compensation (I4), not statically as in Ferrite's CPS encoding
(Chen/Balzer/Toninho, ECOOP 2022, Theorem 4.3).

No MPST layer. EAct handles multi-session composition bottom-up
(correct binary sessions + correct actor discipline = correct
system). If multi-party protocols are ever needed, an MPST layer
can be added without changing the binary channel substrate.

<!-- Plan 9 was bottom-up too: each file server spoke 9P correctly,
composition emerged from the namespace. No global choreography. -->

### Three error channels

| Channel | Mechanism | Formal rule | Audience |
|---------|-----------|-------------|----------|
| Protocol | ReplyPort, ServiceLost | CLL ⊕ branching | Other participants |
| Control | Flow::Stop / Continue | EAct E-Reset | The looper |
| Crash | panic → catch_unwind | EAct E-Raise | Looper + server |

The channels are disjoint. No Result in the handler API. Handler
methods return Flow. EAct has exactly three actor thread outcomes:
E-Reset (returns value), E-Suspend (installs handler), E-Raise
(annihilation). There is no fourth.

Protocol errors (channel 1) are within-session CLL branching — the
error response IS the protocol. The handler continues.

Control (channel 2) is the handler's lifecycle decision — continue
or stop. EAct E-Reset.

Crash (channel 3) is unrecoverable failure. panic → catch_unwind at
the looper boundary → Drop fires obligation compensation → server
notified. EAct E-Raise → zap propagation.

<!-- BeOS had exactly these three: SendReply(error) for protocol,
QuitRequested() → bool for control, thread death for crash.
Handler methods returned void. -->

---

## Type Vocabulary

### Channel substrate (pane-session)

pane-session uses par's CLL types directly (par is a dependency).
Chan<S, T> uses par's types as phantom state parameters over a
Transport trait:

```rust
/// Session-typed channel over a Transport.
pub struct Chan<S, T: Transport> { ... }

// CLL propositions as session types:
pub struct Send<A, S>;    // ⊗ — output A, continue as S
pub struct Recv<A, S>;    // ⅋ — input A, continue as S
pub struct Select<L, R>;  // ⊕ — internal choice
pub struct Branch<L, R>;  // & — external choice
pub struct End;           // 1/⊥ — session terminated

// Streaming (adapted from par):
pub struct Queue<T, S>;   // send N items of T, then continue as S
pub struct Dequeue<T, S>; // receive items until closed, then S

// Server lifecycle (adapted from par):
pub struct Server<S>;     // accept connections, each gets session S

// N-ary branching (pane-session extension beyond par):
#[derive(SessionEnum)]    // generates choose_*/offer() methods
#[repr(u8)]
enum Operation {
    #[session_tag = 0] CheckBalance,
    #[session_tag = 1] Withdraw,
}

// Duality (CLL negation — compliance for binary sessions):
pub trait HasDual { type Dual; }
// Send<A,S> <-> Recv<A, Dual<S>>, Select <-> Branch, End <-> End
```

Recursion uses native Rust enum types — no Rec/Var combinators.
Channel indirection provides the boxing par relies on.

Transport trait abstracts over unix sockets, TCP, TLS, and
in-memory channels. Serialization via postcard. Chan operations
panic on disconnect — the looper's catch_unwind boundary
converts panics to ExitReason::Failed for the crash channel.

### Message

```rust
/// The universal message contract. Every protocol's message type
/// implements this. What BMessage was — the common currency of
/// the system — but typed.
///
/// Clone is required: messages may be filtered, logged, projected
/// into the namespace, forwarded. Obligation handles are not
/// Message variants — they are delivered via separate callbacks.
pub trait Message:
    Serialize + DeserializeOwned + Clone + Send + 'static {}
```

`#[derive(Message)]` on protocol enums. The trait is a marker —
the bounds ARE the contract.

### Protocol and ServiceId

```rust
pub struct ServiceId {
    pub uuid: Uuid,
    pub name: &'static str,
}

impl ServiceId {
    /// UUIDv5 from reverse-DNS name. Not const fn (SHA-1).
    /// Use service_id! proc-macro for const SERVICE_ID.
    pub fn new(name: &'static str) -> Self { ... }
}

/// A protocol relationship between a pane and a service.
pub trait Protocol {
    const SERVICE_ID: ServiceId;
    type Message: Message;
}
```

Naming convention: `com.pane.*` for framework services,
`com.vendor.*` for third-party.

<!-- Plan 9: ServiceId's UUID is analogous to qid.path (stable,
machine-comparable); the name is the directory entry. -->

### Handles\<P\>: uniform dispatch

```rust
/// A handler that can receive messages from protocol P.
/// In EAct, σ maps session endpoints (s,p) to handler values.
/// Under pane's one-service-per-protocol constraint, each
/// Handles<P> impl corresponds to one σ entry.
pub trait Handles<P: Protocol> {
    fn receive(&mut self, msg: P::Message) -> Flow;
}
```

`#[pane::protocol_handler(P)]` attribute macro on an impl block
generates the `Handles<P>::receive` match from named methods.
Rust's exhaustive match IS the coverage guarantee.

The macro generates two dispatch surfaces:
1. **Value dispatch** — `Handles<P>::receive` match over `P::Message`
   variants (Clone-safe values: Changed, LockDenied, ServiceLost).
2. **Obligation dispatch** — separate typed callbacks for obligation
   handles (lock_granted receives ClipboardWriteLock, completion_request
   receives CompletionReplyPort). These bypass the filter chain.

Both dispatch to the same `&mut H`, same looper thread (I6/I7).

### Handler: lifecycle sugar

```rust
/// Every pane implements this. Lifecycle + messaging.
/// Internally equivalent to Handles<Lifecycle> via blanket impl.
///
/// The handler communicates with the framework through a Messenger
/// stored in its own state (typically `self.messenger`), set up
/// during the PaneBuilder phase — not passed as a dispatch parameter.
/// This keeps dispatch signatures uniform and avoids threading a
/// framework reference through every callback.
pub trait Handler: Send + 'static {
    fn ready(&mut self) -> Flow { Flow::Continue }
    fn close_requested(&mut self) -> Flow { Flow::Stop }
    fn disconnected(&mut self) -> Flow { Flow::Stop }
    fn pulse(&mut self) -> Flow { Flow::Continue }
    fn pane_exited(&mut self, pane: Id,
        reason: ExitReason) -> Flow { Flow::Continue }
    /// Query, not dispatch — returns bool, not Flow. &self for
    /// deadlock freedom. Side effects must happen before returning
    /// true (save in close_requested, not here).
    /// BeOS: BLooper::QuitRequested() → bool.
    fn quit_requested(&self) -> bool { true }
    /// Scripting/automation entry point. PropertyInfo describes
    /// which properties this pane exposes through the namespace.
    /// PropertyInfo definition deferred to routing/scripting design.
    fn supported_properties(&self) -> &[PropertyInfo] { &[] }

    /// Obligation callback: incoming request from another pane.
    /// Payload is type-erased (requester and receiver may have
    /// different types). ServiceId identifies the protocol the
    /// sender used — check before downcasting. Reply is an
    /// obligation — default drops it (sends ReplyFailed).
    fn request_received(&mut self,
        service: ServiceId, msg: Box<dyn Any + Send>,
        reply: ReplyPort) -> Flow
    {
        drop(reply);
        Flow::Continue
    }
}

// Framework-provided blanket:
impl<H: Handler> Handles<Lifecycle> for H {
    fn receive(&mut self, msg: LifecycleMessage) -> Flow {
        match msg {
            LifecycleMessage::Ready => self.ready(),
            LifecycleMessage::CloseRequested => self.close_requested(),
            LifecycleMessage::Disconnected => self.disconnected(),
            LifecycleMessage::Pulse => self.pulse(),
            // ...
        }
    }
}
```

Handler is the zero-cost on-ramp. Every pane has lifecycle.
The developer overrides named methods with defaults — no
attribute macro needed for the common case.

### Flow

```rust
/// Handler control flow. EAct E-Reset: the handler returns
/// control to the looper with a lifecycle decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Stop,
}
```

No Result. Errors are the handler's domain (handle internally
or panic). The looper doesn't receive errors — it receives
lifecycle decisions.

### Pane and PaneBuilder\<H\>

```rust
/// A pane — organized state with an interface for views.
/// Non-generic. Connection identity.
#[must_use]
pub struct Pane {
    id: Id,
    tag: Tag,
    connection: Connection,
    looper_tx: LooperSender,
}

impl Drop for Pane {
    fn drop(&mut self) { /* close connection, server cleanup */ }
}

impl Pane {
    /// Enter the typed setup phase for service registration.
    pub fn setup<H: Handler>(self) -> PaneBuilder<H>;

    /// Closure form — no services. Lifecycle messages only.
    pub fn run(self, f: impl FnMut(&Messenger, LifecycleMessage) -> Flow) -> !;

    /// Struct handler — no services needed.
    pub fn run_with<H: Handler>(self, handler: H) -> !;

    /// Struct handler with display — no services needed.
    /// Display is declared in the handshake (Hello's interests
    /// list), not via DeclareInterest. run_with_display includes
    /// display in the handshake automatically.
    pub fn run_with_display<H: Handler + Handles<Display>>(
        self, handler: H) -> !;
}

/// Setup phase. Generic over H for Handles<P> bounds.
/// Consumed by run_with — the builder pattern where the
/// terminal method both builds and enters the event loop.
#[must_use]
pub struct PaneBuilder<H: Handler> {
    pane: Pane,
    dispatch_table: Vec<ServiceDispatchEntry>,
    registered_services: HashSet<ServiceId>,
    _handler: PhantomData<H>,
}

impl<H: Handler> PaneBuilder<H> {
    /// Open a service. Blocks until InterestAccepted/Declined.
    /// Returns None if the service is unavailable.
    /// Duplicate ServiceId is rejected (panics).
    pub fn open_service<P: Protocol>(&mut self) -> Option<ServiceHandle<P>>
    where H: Handles<P>;

    pub fn run_with(self, handler: H) -> !;
    pub fn run_with_display(self, handler: H) -> !
    where H: Handles<Display>;
}

impl<H: Handler> Drop for PaneBuilder<H> {
    fn drop(&mut self) {
        // Revoke accepted interests. Idempotent with
        // ServiceHandle<P> Drop (both send RevokeInterest).
    }
}
```

<!-- Plan 9: Pane = bare process after rfork. PaneBuilder = namespace
construction (bind/mount). run_with = exec. The looper = the
running process. -->

### Dispatch\<H\>: request/reply

```rust
impl Messenger {
    /// Send a request, register typed reply callback.
    /// By analogy with EAct E-Suspend: installs one-shot entry in σ.
    pub fn send_request<H, R>(
        &self,
        target: &Messenger,
        msg: impl Serialize + Send + 'static,
        on_reply: impl FnOnce(&mut H, &Messenger, R) -> Flow + Send + 'static,
        on_failed: impl FnOnce(&mut H, &Messenger) -> Flow + Send + 'static,
    ) -> CancelHandle
    where H: Handler + 'static, R: DeserializeOwned + Send + 'static;
}

/// Drop = no-op (request completes normally).
/// .cancel(self) = voluntary abort, removes Dispatch entry.
pub struct CancelHandle { ... }
```

Dispatch\<H\> is a `HashMap<(ConnectionId, Token), Entry>` —
the dynamic part of σ. Service dispatch (fn pointers) is the
static part. Both are looper-internal.

Lifecycle (by analogy with EAct):
- **Install** (cf. E-Suspend): send_request installs entry
- **Idle**: entry waits; looper services other sessions
- **Dispatch** (cf. E-React): reply arrives, entry consumed, callback fires
- **Failed** (pane-specific): target drops ReplyPort → on_failed fires
- **Cancelled** (pane-specific): .cancel() removes entry, no callbacks
- **Abandoned** (pane-specific): handler drops → entry dropped without
  callbacks. Safe: dropping a receive can only decrease connectivity.

### Obligation handles

Every obligation-carrying type follows the pattern:
- `#[must_use]` — compiler warns on unused
- Move-only — no Clone
- Single success method consumes — `.commit()`, `.reply()`, `.wait()`
- Drop sends failure terminal

```rust
ReplyPort            // Drop → ReplyFailed
CompletionReplyPort  // Drop → failure
ClipboardWriteLock   // Drop → Revert
ServiceHandle<P>     // Drop → RevokeInterest (idempotent)
Pane                 // Drop → close connection
PaneBuilder<H>       // Drop → revoke accepted interests
CreateFuture         // Drop → cancel pending creation
TimerToken           // Drop → cancel timer
```

Obligation handles are NOT Message variants. They are delivered
via separate typed callbacks generated by the protocol_handler
macro. This matches BeOS (obligations were never BMessage
variants) and is forced by the Serialize bound on Message
(obligation handles contain LooperSender — not serializable).

### ServiceHandle\<P\>

```rust
/// A live connection to a service. Bound to a specific Connection
/// and negotiated version at open time — service map changes
/// affect new opens, not existing handles.
/// (Plan 9 fid semantics: bound at open, mount table changes
/// affect new walks only.)
pub struct ServiceHandle<P: Protocol> {
    service_id: ServiceId,
    connection_id: ConnectionId,
    session_id: u8,
    looper_tx: LooperSender,
    _protocol: PhantomData<P>,
}

impl<P: Protocol> Drop for ServiceHandle<P> {
    fn drop(&mut self) {
        let _ = self.looper_tx.send(LooperMessage::RevokeInterest {
            connection_id: self.connection_id,
            session_id: self.session_id,
        });
    }
}

// Protocol-specific methods on concrete instantiation:
impl ServiceHandle<Clipboard> {
    pub fn request_lock(&self);
    pub fn watch(&self, clipboard: &str);
}
```

### Messenger and ServiceRouter

```rust
/// Scoped pane handle. The pane ID is baked in.
/// (Plan 9: like a fid — resolution happens once at open time;
/// the result is a direct binding, not a name.)
pub struct Handle { ... }

/// Handle + ServiceRouter. Cloneable, Send.
pub struct Messenger { ... }

impl Messenger {
    pub fn set_content(&self, data: &[u8]);
    pub fn set_pulse_rate(&self, duration: Duration) -> TimerToken;
    pub fn set_pointer_policy(&self, policy: PointerPolicy);
    pub fn post_app_message<T: AppPayload>(&self, msg: T);
    // send_request defined above in Dispatch section
}

/// Marker trait for fire-and-forget messages. Requires Clone
/// (prevents smuggling obligation handles through post_app_message).
/// Obligation types (ReplyPort, ClipboardWriteLock) are !Clone
/// and cannot implement AppPayload.
pub trait AppPayload: Clone + Send + 'static {}
```

`set_pulse_rate` returns `TimerToken` — Drop cancels the timer.

ServiceRouter maps ServiceId → Connection. One entry per service.

---

## Wire Protocol

### Framing

```
[length: u32][service: u8][payload: postcard]
```

<!-- Plan 9: 9P uses [size[4] type[1] tag[2] ...] — similar
structure. The service: u8 discriminant multiplexes protocol
handlers on one connection, analogous to 9P's type field. -->

Wire service 0 = Control protocol (implicit, not negotiated).
Other discriminants assigned by the server during DeclareInterest.
256-slot ceiling per connection. Serialization via postcard.
A frame exceeding the negotiated max_message_size is a
connection-level error (not per-service — framing is shared).

### Control protocol

```rust
/// Wire service 0. Postcard-encoded. The first byte of the
/// payload discriminates the sub-protocol (lifecycle, display,
/// connection management).
pub enum ControlMessage {
    // Lifecycle sub-protocol
    Lifecycle(LifecycleMessage),
    // Display sub-protocol (only if declared in handshake)
    Display(DisplayMessage),
    // Connection management (framework-internal)
    DeclareInterest { service: ServiceId, expected_version: u32 },
    InterestAccepted { service_uuid: Uuid, session_id: u8, version: u32 },
    InterestDeclined { service_uuid: Uuid, reason: DeclineReason },
    ServiceTeardown { service: u8, reason: TeardownReason },
    RevokeInterest { session_id: u8 },
    Cancel { token: u64 },
}
```

The looper demuxes: Lifecycle variants → Handler methods (via
blanket Handles\<Lifecycle\>), Display variants → Handles\<Display\>
::receive, connection-management variants → framework-internal
handling. The developer never sees ControlMessage directly.

Implicit — never DeclareInterest'd.

### Capability declaration

Display declared in handshake (Hello's interests list). Other
services declared via DeclareInterest in active phase. Server
assigns session-local u8 per accepted service.

```rust
// In ClientToServer:
DeclareInterest { service: ServiceId, expected_version: u32 }

// In ServerToClient:
InterestAccepted { service_uuid: Uuid, session_id: u8, version: u32 }
InterestDeclined { service_uuid: Uuid, reason: DeclineReason }
// DeclineReason: VersionMismatch, ServiceUnknown
// No downgrade negotiation — client retries with a different
// version or accepts unavailability.
```

RevokeInterest when ServiceHandle drops. Idempotent. In-flight
messages for revoked services discarded by looper.

### Handshake

```
Client → Server: Hello { version, max_message_size, interests }
Server → Client: Welcome { version, instance_id, max_message_size, bindings }
```

<!-- Plan 9: factotum handled authentication outside the
application. pane pushes further — the application doesn't even
see an auth conversation. Identity is a transport property. -->

PeerAuth derived from transport: `Kernel { uid, pid }` for unix
(SO_PEERCRED), `Certificate { subject, issuer }` for TLS. Identity
is transport-level, not carried in Hello.

### Request cancellation

```rust
Cancel { token: u64 }  // advisory, same semantics as 9P Tflush
```

### ProtocolAbort

`Chan<S, T>` Drop sends `[0xFF][0xFF]` on the transport. Peer
frees session thread immediately. Best-effort (`let _ = ...`).
Checked at framing layer before postcard deserialization (I11).

---

## App and Connections

### App: the entry point

```rust
impl App {
    /// Connect to the primary server. Panics if the server
    /// is unreachable — connection failure is infrastructure
    /// misconfiguration, not a recoverable application error.
    pub fn connect(signature: &str) -> App;
    /// Create a pane. Panics on server rejection.
    pub fn create_pane(&self, tag: Tag) -> CreateFuture;
}
```

Pre-run failures (connect, create_pane) panic because they are
infrastructure: the service map is wrong, the server is down,
the handshake was rejected. These are deployment problems, not
application logic. The supervisor (s6, systemd) restarts the
process. `open_service` returns `Option` instead of panicking
because optional services are a legitimate application concern.

<!-- BeOS: BApplication(signature) called debugger() on init
failure, or exit(0) if no error output parameter. The
constructor WAS the infrastructure check. -->

### Multi-server

App connects to multiple servers. Each provides different
capabilities. No server is a mandatory intermediary.

Typical topology:
- Compositor on machine B (Display, Input)
- Clipboard service on machine C
- Registry on machine D

App resolves servers from the service map and connects lazily.
Each connection is an internal calloop source. The developer
sees App, Pane, PaneBuilder\<H\>, Messenger, ServiceHandle\<P\>.

### Service map

```
# $PANE_SERVICES or /etc/pane/services.toml
[compositor]
uri = "unix:///run/pane/compositor.sock"

[clipboard]
uri = "tcp://clipboard.internal:9090"
tls = true
```

Precedence: `$PANE_SERVICE_OVERRIDES` > manifest >
`$PANE_SERVICES` > `/etc/pane/services.toml`.

### Per-connection failure isolation

Connection going down affects only its capabilities. Other
connections unaffected. Compositor lost → can't display.
Clipboard lost → copy/paste returns error. Registry lost →
discovery fails. The pane continues with remaining capabilities.

<!-- Plan 9: Ehangup per-connection. Failure surfaces at the
use site, not as a global event. -->

### Cross-connection ordering

Events within a connection are FIFO (TCP). Events across
connections are not causally ordered. The unified batch
imposes a total order for dispatch purposes, but cross-
connection ordering within a batch is implementation-defined
and must not be relied upon. Handlers that need cross-
connection causality use send_request callbacks.

### Remote connections require TLS

`PeerAuth::Certificate { subject, issuer }` for TLS.
Plaintext TCP not supported. `pane dev-certs` for development.

---

## Filter Chain

<!-- BeOS: BMessageFilter on BHandler/BLooper. Filters saw the
raw BMessage* including embedded reply ports — pane corrects this
by separating obligations from filterable messages. -->

Filters are typed per-protocol. The `Message` trait requires
`Clone` (supertrait), which makes it not object-safe — `&dyn
Message` is ill-formed in Rust. This is by design: filters
operate on concrete protocol message types, not trait objects.

```rust
pub trait MessageFilter<M: Message>: Send + 'static {
    fn filter(&mut self, msg: &M) -> FilterAction<M>;
    fn matches(&self, msg: &M) -> bool { true }
}

pub enum FilterAction<M> {
    Pass,
    Transform(M),
    Consume,
}
```

The base filter chain is `MessageFilter<LifecycleMessage>` —
every pane has lifecycle events to filter (shortcut transforms,
exit monitoring, etc.).

Per-service filter hooks are typed by the service's message
type: `MessageFilter<ClipboardMessage>`, `MessageFilter<
DisplayMessage>`, etc. Registered at `open_service` time.
Keyed by `ServiceId`.

Obligation handles bypass all filters — they are not `Message`
variants and dispatch directly to their typed callbacks.

Filters run in registration order. `add_filter` appends.
`FilterHandle` Drop removes. Earlier filters see originals;
later filters see transforms.

---

## Service Registration

<!-- BeOS: BApplication's constructor connected to app_server
synchronously, ran the handshake, and blocked until registration
completed — all before Run() started the message loop. Same
structural guarantee here. -->

Services opened during PaneBuilder\<H\> setup phase:

1. Resolve capability to a Connection (service map)
2. Send DeclareInterest (blocking — pre-looper, I2/I8 don't apply)
3. Wait for InterestAccepted/Declined
4. On acceptance: capture monomorphized Handles\<P\>::receive fn
   pointer, register calloop source
5. Return `Some(ServiceHandle<P>)` or `None` on decline

All open_service calls resolve before run_with starts the looper.
No race between interest confirmation and dispatch — enforced
structurally by PaneBuilder consumption.

---

## Dispatch (looper internals)

### The looper

calloop-backed event loop. Dispatches events sequentially on one
thread (I6). The looper is generic over H internally — the
generic parameter is encapsulated.

### catch_unwind boundary

```rust
match catch_unwind(AssertUnwindSafe(|| {
    handler.receive(msg)
})) {
    Ok(Flow::Continue) => { /* next event */ }
    Ok(Flow::Stop) => {
        drop(handler);
        exit(0);  // ExitReason::Graceful
    }
    Err(panic_payload) => {
        drop(handler);
        // ExitReason::Failed → PaneExited broadcast
        exit(101);
    }
}
```

AssertUnwindSafe justified: handler is never re-used after
caught panic. I1 (`panic = unwind`) is load-bearing.

`exit()` is `std::process::exit` — a hard exit that does not
run Drop for stack frames above the looper. This is intentional:
the handler is explicitly dropped before exit (obligation
compensation fires), and run_with returns `-> !` so nothing
meaningful exists above it on the stack.

### ExitReason (looper-internal, not in handler API)

```rust
/// Broadcast to other panes via PaneExited. Stripped of
/// error details — failure reason is private to the process.
pub enum ExitReason {
    Graceful,       // Flow::Stop
    Disconnected,   // primary connection lost
    Failed,         // caught panic
    InfraError,     // calloop/socket/framing
}
```

### Batch processing

Each calloop cycle: collect events from all sources into a
unified batch. Total ordering within the batch. Coalescing
within the batch (mouse events, etc.). Base filter chain sees
Message variants. Service events dispatch through Handles\<P\>.
Obligation handles dispatch through separate callbacks.

---

## Linear Discipline

### Typestate handles

Every obligation-carrying type: `#[must_use]`, move-only,
single success method consumes, Drop sends failure terminal.

### Affine gap

Rust is affine (values can be dropped), not linear (values must
be consumed). Drop impls compensate by sending failure terminals.

Residual risks where Drop cannot fire: double panic (abort),
`std::process::abort()`, SIGKILL, OOM-triggered abort. The
server detects these via fd hangup (EPOLLHUP on unix, TCP RST
on remote) and cleans up — the backstop when I1 is violated.

### Destruction sequence

Three triggers, same sequence:

- **Flow::Stop** from any handler callback → ExitReason::Graceful
- **Primary connection lost** → looper calls disconnected() →
  if handler returns Flow::Stop → ExitReason::Disconnected
- **Caught panic** → ExitReason::Failed

The sequence:
1. dispatch.fail_connection() (per S4) — fires on_failed for
   Dispatch entries keyed to lost Connection. on_failed callbacks
   receive `&mut H` and can update handler state, but must NOT
   call send_request (new entries created during destruction are
   abandoned per the "Abandoned" lifecycle — cleared without
   callbacks when the handler drops in step 2).
2. dispatch.clear() — remaining entries across all Connections
   dropped without callbacks.
3. Handler dropped — obligation handles fire Drop compensation
   (ReplyPort → ReplyFailed, ClipboardWriteLock → Revert,
   ServiceHandle → RevokeInterest)
4. Server notified via PaneExited { reason }

### send_and_wait

Synchronous blocking variant of send_request. Must NOT be called
from a handler method. Enforced at runtime: looper thread-local
check, panic on violation (I8).

---

## Invariants

### System invariants

- **I1**: `panic = unwind` in all pane binaries (Drop must fire).
  Backstop: fd hangup detection.
- **I2**: No blocking calls in handler methods (EAct Global
  Progress — requires handler termination).
- **I3**: Handler callbacks terminate (return Flow).
- **I4**: Typestate handles: `#[must_use]` + Drop compensation.
- **I5**: Filters see only Clone-safe Message variants.
  Obligation handles bypass filters.
- **I6**: Sequential single-thread dispatch per pane.
- **I7**: Service dispatch fn pointers called sequentially,
  preserving `&mut H` exclusivity.
- **I8**: `send_and_wait` panics from looper thread.
- **I9**: Dispatch cleared before handler drop.
- **I10**: Chan Drop must not block (best-effort write).
- **I11**: ProtocolAbort `[0xFF][0xFF]` checked at framing layer
  before deserialization.
- **I12**: Unknown service discriminant → connection-level error.
- **I13**: open_service blocks until InterestAccepted/Declined.
  ServiceHandle represents a confirmed binding.

### Dispatch entry invariants

- **S1**: Token uniqueness (AtomicU64, per-Connection namespace).
- **S2**: Sequential dispatch — callbacks share `&mut H`, never
  concurrent (follows from I6/I7).
- **S3**: Control-before-events — RegisterRequest processed before
  Reply in same batch.
- **S4**: On Connection loss, fail_connection() fires for entries
  keyed to that Connection only, before disconnected(). On handler
  destruction, dispatch.clear() drops entries without callbacks.
- **S5**: Cancel removes entry without firing callbacks.
- **S6**: `panic = unwind` (follows from I1).

---

## Developer Experience

### Minimal headless agent

```rust
use pane_app::{App, Tag, Handler, Messenger, Flow};

struct StatusAgent {
    messenger: Messenger,
}

impl Handler for StatusAgent {
    fn ready(&mut self) -> Flow {
        self.messenger.set_content(b"online");
        self.messenger.set_pulse_rate(Duration::from_secs(60));
        Flow::Continue
    }

    fn pulse(&mut self) -> Flow {
        let status = check_health();
        self.messenger.set_content(status.as_bytes());
        Flow::Continue
    }
}

fn main() {
    let app = App::connect("com.ops.status");
    let pane = app.create_pane(Tag::new("Server Status")).wait();
    let mut builder = pane.setup::<StatusAgent>();
    let messenger = builder.messenger();
    builder.run_with(StatusAgent { messenger })
}
```

### Display editor with clipboard

```rust
use pane_app::*;

struct Editor {
    messenger: Messenger,
    buffer: String,
    clipboard: ServiceHandle<Clipboard>,
}

impl Handler for Editor {
    fn ready(&mut self) -> Flow {
        self.messenger.set_content(self.buffer.as_bytes());
        Flow::Continue
    }

    fn close_requested(&mut self) -> Flow {
        Flow::Stop
    }
}

#[pane::protocol_handler(Display)]
impl Editor {
    fn key(&mut self, event: KeyEvent) -> Flow {
        self.buffer.push(event.char);
        self.messenger.set_content(self.buffer.as_bytes());
        Flow::Continue
    }

    fn command_executed(&mut self, cmd: &str, _: &str) -> Flow {
        if cmd == "copy" {
            self.clipboard.request_lock();
        }
        Flow::Continue
    }
}

// Obligation callbacks — separate from ClipboardMessage values:
#[pane::protocol_handler(Clipboard)]
impl Editor {
    // Value messages (Clone-safe, filter-visible):
    fn changed(&mut self, _: &str, _: Id) -> Flow {
        Flow::Continue
    }
    fn lock_denied(&mut self, _: &str, _: &str) -> Flow {
        Flow::Continue
    }
    fn service_lost(&mut self) -> Flow {
        Flow::Continue
    }

    // Obligation callback (NOT a ClipboardMessage variant):
    fn lock_granted(&mut self, lock: ClipboardWriteLock) -> Flow
    {
        lock.commit(self.buffer.as_bytes().to_vec(), ClipboardMetadata {
            content_type: "text/plain".into(),
            sensitivity: Sensitivity::Normal,
            locality: Locality::Any,
        });
        Flow::Continue
    }
}

fn main() {
    let app = App::connect("com.pane.editor");
    let pane = app.create_pane(
        Tag::new("Editor")
            .command(cmd("copy", "Copy").shortcut("Ctrl+C")),
    ).wait();

    let mut builder = pane.setup::<Editor>();
    let messenger = builder.messenger();
    let clipboard = builder.open_service::<Clipboard>()
        .expect("clipboard service required");
    builder.run_with_display(Editor {
        messenger,
        buffer: String::new(),
        clipboard,
    })
}
```

### Closure form

```rust
fn main() {
    let app = App::connect("com.example.hello");
    let pane = app.create_pane(Tag::new("Hello")).wait();
    pane.run(|msg| match msg {
        LifecycleMessage::CloseRequested => Flow::Stop,
        _ => Flow::Continue,
    })
}
```

---

## Framework Protocols

All use Protocol + Handles\<P\> + `#[pane::protocol_handler]`.

```rust
struct Lifecycle;
impl Protocol for Lifecycle {
    const SERVICE_ID: ServiceId = service_id!("com.pane.lifecycle");
    type Message = LifecycleMessage;
}

struct Display;
impl Protocol for Display {
    const SERVICE_ID: ServiceId = service_id!("com.pane.display");
    type Message = DisplayMessage;
}

struct Clipboard;
impl Protocol for Clipboard {
    const SERVICE_ID: ServiceId = service_id!("com.pane.clipboard");
    type Message = ClipboardMessage;
}

struct Routing;
impl Protocol for Routing {
    const SERVICE_ID: ServiceId = service_id!("com.pane.routing");
    type Message = RoutingMessage;
}
```

Lifecycle and Display are bundled in the Control protocol (wire
service 0, implicit — declared in the handshake's interests list,
not via DeclareInterest). Both have Protocol impls with ServiceIds
for type-system dispatch, but they share the Control wire channel.
Other services get negotiated discriminants via DeclareInterest.

---

## Application-Defined Protocols

Applications define their own Protocol with a custom Message enum.
The same `Protocol + Handles<P>` mechanism handles both framework
and application protocols:

```rust
struct ModelProtocol;
impl Protocol for ModelProtocol {
    const SERVICE_ID: ServiceId = service_id!("com.example.editor.model");
    type Message = ModelMessage;
}

#[derive(Message, Clone, Serialize, Deserialize)]
enum ModelMessage {
    Completion { cursor: usize, text: String },
    DiagnosticReady { path: String, diagnostics: Vec<Diagnostic> },
    IndexingProgress { done: u32, total: u32 },
}

#[pane::protocol_handler(ModelProtocol)]
impl Editor {
    fn completion(&mut self, cursor: usize, text: String) -> Flow { ... }
    fn diagnostic_ready(&mut self, ...) -> Flow { ... }
    fn indexing_progress(&mut self, done: u32, total: u32) -> Flow { ... }
}
```

Application protocol messages are local to the looper, never cross
the wire, typed at compile time with exhaustive dispatch via the
attribute macro. `post_app_message` is the simpler alternative for
one-off fire-and-forget notifications that don't warrant a full
Protocol definition.

---

## Obligation Handle Lifetime

Obligation handles (ReplyPort, CompletionReplyPort,
ClipboardWriteLock) should be consumed within the callback
invocation that receives them. Storing an obligation handle in
`self` defers its resolution until handler destruction, blocking
the requesting pane indefinitely.

This is a convention, not a type-level enforcement. `ReplyPort`
remains `Send` (worker threads may need to reply). If a handler
stores and forgets, Drop compensation (ReplyFailed) fires at
handler destruction — the linear discipline works, but the
requester waits longer than necessary.

---

## Streaming and Backpressure

Queue<T, S> / Dequeue<T, S> for session-typed streaming:

```
Wire: 0x00 + postcard(value) = Item, continue streaming
      0x01                   = Closed, transition to continuation S
```

Backpressure: stream items buffer into the ConnectionSource's
write buffer. If the write buffer hits the high-water mark,
push() returns Err(Backpressure) — does NOT block (I2). The
handler decides: drop the item, stop producing, or return
Flow::Stop. OS-level flow control provides additional backpressure.

Streams must be closed before session suspension. The server
initiates suspension by sending a suspend signal; the client
must close open streams before acknowledging. A resumed session
with a desynchronized stream is a protocol error.

---

## Session Suspension and Resumption

Server issues a serializable session token on suspend. Client
holds the token across disconnection. On reconnect, client
presents the token; server locates suspended state. Stateful
obligations do NOT survive suspension — they fail via Drop
compensation. The resumed session re-declares interests and
re-negotiates per-service protocol versions.

Session tokens expire server-side. Multi-server suspension is
per-Connection, independent.

<!-- Plan 9: analogous to aan(8) — authenticated, anti-replay
session layer that maintains sessions across network disruption.
pane's suspension is explicit (not transparent) to avoid masking
latency changes. -->

---

## Maximum Message Size

Hello/Welcome negotiates `max_message_size`. Both sides send
their maximum; the effective limit is the minimum. Default: 16MB.
The server rejects frames exceeding this. The `length` field in
`[length: u32][service: u8][payload]` counts the service byte
plus payload. The length field does NOT include itself.

---

## Open Questions

1. **Pane identity across servers.** Who assigns pane Id? Is it
   global (one Id, presented to all servers) or connection-local?
   Multi-server topology needs an answer.

2. **Reconnection.** A lost service is currently lost for the
   pane's lifetime (PaneBuilder is consumed, no mid-session
   open_service). Explicit reconnection (exit and re-launch) is
   the current model. Transparent reconnection is deferred.

3. **Same-server service routing.** When two services resolve to
   the same server, does the App open one connection or two?
   One connection is more efficient; two connections simplify
   failure isolation.

---

## Design Principles

### Functoriality

Build the full architecture's types from day one. No simplified
types that assume structure doesn't exist — a type that omits
structure creates patterns that can't cleanly accommodate the
full design.

`ServiceRouter` with one entry, not a bare sender.
`ServiceId { uuid, name }`, not a bare string.
`Dispatch` keyed by `(ConnectionId, token)`, not bare token.

<!-- BeOS lesson: string-based app signatures shaped an ecosystem
built on strcmp(). When structured identity was needed (launch
daemon, package management), everything was string-comparison
all the way down. -->

### send_and_wait

Synchronous blocking variant of send_request. Must NOT be called
from a handler method. Enforced at runtime: looper thread-local
check, panic on violation (I8). Same-Connection cycles (pane A →
pane B → pane A through one server) remain a runtime hazard.
Detection is not implemented — the framework does not track
cross-pane dependency graphs. Mitigation: timeouts on
send_and_wait, and documentation of the mutual-deadlock risk.

Tokens are per-Connection (three servers = three namespaces).
Token allocation is internal to the framework.

---

## Resolved Questions

1. **Handler returns Flow, not Result.** Three error channels are
   disjoint. Handler owns its error domain. Looper receives lifecycle
   decisions, not errors.

2. **Message: Clone.** Obligation handles are not Message variants.
   Forced by Serialize bound (obligation handles are !Serialize).
   Matches BeOS (BMessage was pure values; obligations used separate
   mechanisms).

3. **par as CLL substrate.** par is complete (per Strba). par is a
   direct dependency of pane-session. Chan<S, T> uses par's types
   (par::exchange::Send, par::exchange::Recv, etc.) as phantom state
   parameters over pane-session's Transport trait. par::Dual
   provides duality checking. Chan panics on disconnect (same model
   as par).

4. **EAct, not MPST.** Bottom-up composition (correct binaries +
   correct actors = correct system). MPST can be added later for
   multi-party protocols without changing the channel substrate.

5. **No phases.** The spec describes the full architecture.
   Implementation order in PLAN.md.

6. **open_service returns Option.** Service may not be available.
   None = service unavailable, Some = confirmed binding.

7. **Native recursion.** No Rec/Var. Par's approach: Rust enum
   recursion with channel indirection. Queue and Server are the
   named patterns.

8. **PaneBuilder\<H\>.** Pane is non-generic. Setup phase introduces
   H when services are needed. Consumed by run_with.

9. **No special handler traits.** DisplayHandler, RoutingHandler
   eliminated. All protocols use Handles\<P\>.

10. **ExitReason is looper-internal.** Not in the handler API.
    PaneExited broadcast carries stripped disposition tags.
