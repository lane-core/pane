# pane Architecture

A pane is organized state with views of that state. Display is
one view. The namespace at `/pane/` is another. Both project
the same state; optic laws keep them consistent. Headless is
the base case. Display is opt-in.

---

## Layers

```
par (Strba)        session-typed binary channels, duality
pane-session       bridges par to IPC transports (postcard, unix/tcp/tls)
pane-app           single-threaded dispatch, request/reply, service binding
```

**pane-session.** The handler calls par's Send::send() and
Recv::recv() directly. Bridge threads serialize between par's
oneshot channels and the Transport trait. Par drives the
handshake; the active phase uses calloop and typed enum dispatch.

Connection is two-phase. Phase 1 verifies the transport and
returns Result — common failures (server not running) are caught
here. Phase 2 runs the par handshake. Transport death mid-
handshake panics; the session is aborted.

**pane-app.** Each pane runs on one thread. The looper dispatches
messages sequentially — one at a time, never concurrent. Protocol
bindings are registered at setup (fn pointers captured by
open_service). Reply callbacks are installed per-request
(send_request) and consumed on reply.

**Optics.** Reading `/pane/<n>/attrs/cursor` projects handler state
through a monadic lens. Writing `cursor 42` to `/pane/<n>/ctl`
routes through the same lens's setter. The optic laws (GetPut,
PutGet, PutPut) guarantee read/write consistency — the ctl write
path and the namespace read path use the same fn pointer.

A monadic lens has a pure view and an effectful set that returns
`Vec<Effect>`. Effects (compositor notifications, content
updates) are executed by the framework after state mutation,
before snapshot publication. Lifecycle commands (`close`) and
IO-first commands (`reload`) bypass the optic layer and dispatch
to a freeform handler method. Details in `docs/optics-design-brief.md`.

### Error channels

| Channel | Mechanism | Audience |
|---------|-----------|----------|
| Protocol | ReplyPort, ServiceLost | Other participants |
| Control | Flow::Stop / Continue | The looper |
| Crash | panic → catch_unwind | Looper + server |

The channels are disjoint. Handler methods return Flow. Protocol
errors are valid protocol messages — the handler continues.
Control is the handler's lifecycle decision. Crash is
unrecoverable — the looper catches the panic, fires Drop
compensation, and notifies the server.

### Composition

Each binary session's endpoints match (par's duality). Single-
threaded dispatch prevents inter-session deadlocks. If par's
session types are consumed (not dropped), the protocol follows
the declared structure. `#[must_use]` and Drop compensation
handle the affine gap.

Each connection is bilateral. The actor composes multiple
connections by dispatching their events sequentially.

---

## Types

### Sessions (pane-session)

```rust
type ClientHandshake = par::exchange::Send<Hello, par::exchange::Recv<Welcome>>;
type ServerHandshake = par::Dual<ClientHandshake>;

let client = bridge_client_handshake(transport);
let client = client.send(hello);
let welcome = block_on(client.recv1());
```

Par provides Send, Recv, Enqueue, Dequeue, Server, Proxy,
Session (duality), and Dual. Branching uses Rust enums.
Recursion uses Rust enum types. Transport abstracts over unix
sockets, TCP, TLS, and in-memory channels.

### Message

```rust
pub trait Message:
    Serialize + DeserializeOwned + Clone + Send + 'static {}
```

Blanket impl — any type satisfying the bounds is a Message.
Obligation handles (ReplyPort, ClipboardWriteLock) are not
Message types. They are delivered via separate callbacks.

### Protocol and ServiceId

```rust
pub struct ServiceId {
    pub uuid: Uuid,
    pub name: &'static str,
}

pub trait Protocol {
    fn service_id() -> ServiceId;
    type Message: Message;
}
```

ServiceId uses UUIDv5 derived from the reverse-DNS name.
Framework services: `com.pane.*`. Third-party: `com.vendor.*`.

### Handles\<P\>

```rust
pub trait Handles<P: Protocol> {
    fn receive(&mut self, msg: P::Message) -> Flow;
}
```

One impl per protocol. The `#[pane::protocol_handler(P)]` macro
generates the match from named methods. Exhaustive match is the
coverage guarantee.

The macro generates two dispatch paths: value messages through
`Handles<P>::receive`, obligation handles through separate typed
callbacks. Both dispatch to the same `&mut H` on the looper
thread.

### Handler

```rust
pub trait Handler: Send + 'static {
    fn ready(&mut self) -> Flow { Flow::Continue }
    fn close_requested(&mut self) -> Flow { Flow::Stop }
    fn disconnected(&mut self) -> Flow { Flow::Stop }
    fn pulse(&mut self) -> Flow { Flow::Continue }
    fn pane_exited(&mut self, pane: Id, reason: ExitReason) -> Flow { Flow::Continue }
    fn quit_requested(&self) -> bool { true }
    fn supported_properties(&self) -> &[PropertyInfo] { &[] }
    fn request_received(&mut self, service: ServiceId,
        msg: Box<dyn Any + Send>, reply: ReplyPort) -> Flow {
        drop(reply); Flow::Continue
    }
}
```

A blanket impl maps Handler to Handles\<Lifecycle\>. The
developer overrides named methods. The handler communicates
with the framework through a Messenger it stores in its own
fields, set up during the PaneBuilder phase.

### Flow

```rust
pub enum Flow { Continue, Stop }
```

Both variants are normal completion. Errors are the handler's
domain — handle internally or panic.

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
    /// Installs a one-shot reply callback in the handler store.
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
the per-request part of the handler store. Service dispatch (fn
pointers) is the per-protocol part. Both are looper-internal.

Lifecycle of a Dispatch entry:
- **Install**: send_request installs entry
- **Idle**: entry waits; looper services other sessions
- **Dispatch**: reply arrives, entry consumed, callback fires
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
[length: u32 LE][service: u8][payload: postcard]
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

When a bridge thread's par session endpoint is dropped mid-
protocol (transport failure during handshake), ProtocolAbort
is sent as a normal length-prefixed frame `[length: u32 = 1]
[service: 0xFF]`. Service discriminant 0xFF is reserved —
never assigned by DeclareInterest. The framing layer checks
service == 0xFF after reading a complete frame. Best-effort
(`let _ = ...`). Checked at framing layer before postcard
deserialization (I11).

---

## App and Connections

### App: the entry point

```rust
impl App {
    /// Connect to the primary server. Panics if the server
    /// is unreachable.
    pub fn connect(signature: &str) -> App;
    /// Create a pane. Panics on server rejection.
    pub fn create_pane(&self, tag: Tag) -> CreateFuture;
}
```

`connect` and `create_pane` panic on failure — the service map
is wrong, the server is down, or the handshake was rejected.
The supervisor (s6, systemd) restarts the process.
`open_service` returns `Option`: a missing service is a normal
condition.

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
connections have no causal order. The unified batch imposes
a total order for dispatch, but cross-connection ordering
within a batch is implementation-defined. Handlers that need
cross-connection causality use send_request callbacks.

### Remote connections require TLS

`PeerAuth::Certificate { subject, issuer }` for TLS.
Plaintext TCP not supported. `pane dev-certs` for development.

---

## Filter Chain

<!-- BeOS: BMessageFilter on BHandler/BLooper. Filters saw the
raw BMessage* including embedded reply ports — pane corrects this
by separating obligations from filterable messages. -->

Filters are typed per-protocol. `Message` requires `Clone`,
which makes it not object-safe — `&dyn Message` is ill-formed
in Rust. Filters operate on concrete protocol message types.

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

AssertUnwindSafe is sound: the handler is dropped after a
caught panic, never re-used. I1 (`panic = unwind`) is
load-bearing.

`exit()` is `std::process::exit` — a hard exit that skips Drop
for stack frames above the looper. The handler is explicitly
dropped before exit (obligation compensation fires). `run_with`
returns `-> !`, so nothing meaningful exists above it.

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

## Namespace (pane-fs)

<!-- Plan 9: /proc per-process synthetic files. rio: /dev/wsys
per-window synthetic files. pane-fs is the same idea — each
pane gets a directory in /pane/ with structured entries. The
filesystem IS the scripting and test interface. -->

Each pane appears as a directory under `/pane/`:

```
/pane/json              all panes as JSON array
/pane/<id>/tag          title text (read-only)
/pane/<id>/body         content (semantic, not rendered)
/pane/<id>/attrs/<name> named attributes via monadic lenses
/pane/<id>/attrs/json   all attrs from one snapshot as JSON
/pane/<id>/ctl          line-oriented command interface
/pane/<id>/json         full pane state as JSON object
```

`json` is a reserved filename at every directory level —
same pattern as `tag`, `body`, `ctl`. Each `json` file
returns a structured snapshot of its parent directory in one
FUSE read.

### Snapshot model

The handler state lives on the looper thread (`&mut self`).
pane-fs reads from a FUSE thread. The looper publishes a
Clone'd state snapshot after each dispatch cycle. FUSE threads
read from the snapshot via ArcSwap (zero-contention atomic
swap). Reads never block the looper.

Per-pane snapshot consistency: all attributes read within one
FUSE operation come from the same dispatch cycle. `attrs.json`
extends this to cross-attribute reads — one FUSE read, one
snapshot, all attributes as a JSON object with string values.

### Ctl writes

Ctl writes are synchronous. The FUSE write blocks until the
looper processes the command and publishes the updated snapshot.
A read after a ctl write sees the effect. This is the Plan 9
model — devproc.c, rio's wctl.c, acme's xfidctlwrite all
block writes until the command takes effect.

Mechanism: FUSE write handler sends (command, oneshot_tx) to
the looper via calloop channel, blocks on oneshot_rx. The
looper processes the command, executes effects, publishes the
snapshot, sends the result on oneshot_tx.

Multi-line writes process sequentially, stop on first error,
return bytes consumed up to the error. Error reporting via
FUSE errno: EINVAL (bad syntax), EIO (handler error/panic),
ENXIO (pane exited), ETIMEDOUT (5s timeout).

### Ctl dispatch

State-mutating commands (`cursor 42`, `set-tag "foo"`, `goto`,
`focus`) route through the monadic lens layer. The dispatcher
parses the command, looks up the attribute by name, and calls
the monadic setter. This eliminates wiring divergence by
construction — the same fn pointer serves both the read path
(AttrReader) and the write path (ctl).

Lifecycle commands (`close`) and IO-first commands (`reload`)
bypass optics and dispatch to `ctl_fallback()` on the handler.

### Failure model

A crashed pane's namespace entry is removed immediately — not
left stale. Concurrent reads in flight return EIO. New reads
return ENOENT.

### Namespace as test surface

Seven invariants are directly testable through the namespace:
I1, I4, I6, I8, I9, I13, S4. Three of these (I9, I13,
I6-through-snapshots) are testable ONLY through the namespace —
unit tests on dispatch.rs cannot cover the publication
boundary where looper state becomes externally observable.

---

## Linear Discipline

### Typestate handles

Every obligation-carrying type: `#[must_use]`, move-only,
single success method consumes, Drop sends failure terminal.

### Drop compensation

Rust allows values to be dropped without consuming them. Drop
impls compensate by sending failure terminals.

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
   receive `&mut H` and can update handler state. They must not
   call send_request — entries created during destruction are
   cleared without callbacks when the handler drops in step 2.
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
- **I2**: No blocking calls in handler methods. The looper
  services all session endpoints on one thread; a blocked
  handler stalls them all.
- **I3**: Handler callbacks terminate (return Flow).
- **I4**: Typestate handles: `#[must_use]` + Drop compensation.
- **I5**: Filters see only Clone-safe Message variants.
  Obligation handles bypass filters.
- **I6**: Sequential single-thread dispatch per pane.
- **I7**: Service dispatch fn pointers called sequentially,
  preserving `&mut H` exclusivity.
- **I8**: `send_and_wait` panics from looper thread.
- **I9**: Dispatch cleared before handler drop.
- **I10**: ProtocolAbort on session drop must not block (best-effort write).
- **I11**: ProtocolAbort uses reserved service discriminant 0xFF,
  checked at framing layer before deserialization.
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
    fn service_id() -> ServiceId { ServiceId::new("com.pane.lifecycle") }
    type Message = LifecycleMessage;
}

struct Display;
impl Protocol for Display {
    fn service_id() -> ServiceId { ServiceId::new("com.pane.display") }
    type Message = DisplayMessage;
}

struct Clipboard;
impl Protocol for Clipboard {
    fn service_id() -> ServiceId { ServiceId::new("com.pane.clipboard") }
    type Message = ClipboardMessage;
}

struct Routing;
impl Protocol for Routing {
    fn service_id() -> ServiceId { ServiceId::new("com.pane.routing") }
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
    fn service_id() -> ServiceId { ServiceId::new("com.example.editor.model") }
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

Consume obligation handles (ReplyPort, CompletionReplyPort,
ClipboardWriteLock) within the callback that receives them.
Storing one in `self` defers resolution until handler
destruction — the requesting pane blocks indefinitely.

Convention, not type-level enforcement. `ReplyPort`
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

### Full-structure types

Types carry the full architecture's structure from the start.
A type that omits structure forces patterns that break when
the full design arrives.

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
The framework does not track cross-pane dependency graphs.
Timeouts on send_and_wait bound the deadlock window.

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

3. **par as session substrate.** par is complete (per Strba). par is
   a direct dependency of pane-session. The handler uses par's
   native Send::send() and Recv::recv() API. Bridge threads
   mediate between par's oneshot channels and the Transport.
   par::Dual provides duality checking. Sessions panic on
   disconnect (same model as par). Two-phase connect: Phase 1
   verifies transport (Result), Phase 2 runs par handshake.

4. **Bottom-up composition.** Correct binary sessions + correct
   actor discipline = correct system. Each connection is bilateral;
   the actor composes them. Multi-party protocols can be added
   later without changing the channel substrate.

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
