# pane v2 Architecture

A pane is organized state with an interface that allows views of
that state. Display is one view. Filesystem projection is another.
Scripting queries, protocol endpoints, remote agent access — all
views of the same state, kept consistent by optic laws.

Designed from first principles after two weeks of proof-of-concept
development. The proof of concept validated the API vocabulary,
mapped the subsystem landscape, and revealed that pane-headless
(a server with no display running the same protocol) is the
clarifying constraint: pane is a protocol framework that happens
to have a display mode, not a display framework that also works
headless.

Designed with input from be-systems-engineer, plan9-systems-engineer,
and session-type-consultant.

---

## Theoretical Foundation

**EAct** (Fowler et al., "Safe Actor Programming with Multiparty
Session Types") is the governing formalism. It reconciles pane's
two design lineages:

- **BeOS**: EAct formalizes what Be's API achieved implicitly.
  BLooper = EAct actor with sequential handler invocation. BHandler
  = handler store σ. BMessenger = session endpoint. BMessageFilter
  = channel transformer. The formalism proves why Be worked and
  reveals where it was unsound (handler chain re-entrancy, untyped
  dispatch, no coverage checking).

- **Plan 9**: Optic discipline (Clarke et al.) formalizes what
  Plan 9 achieved through filesystem convention. /dev/snarf, rio's
  per-window synthetic files, per-process namespaces — all are
  optic projections. The optic laws (GetPut, PutGet, PutPut)
  guarantee consistency between views. Session types govern protocol
  relationships between components; optics govern state-access
  relationships within and across components.

**The channel discipline**: pane-session's types are CLL-derived
(classical linear logic, in the lineage of Wadler's "Propositions
as Sessions," JFP 2014). CLL governs what a single binary protocol
can express — Send/Recv, duality, branching, streaming. EAct
operates at the layer above — how one actor interleaves work across
multiple sessions. The two compose: EAct's Progress theorem
requires individual protocols to be compliant; pane-session's
typestate encoding mirrors CLL's structure, providing protocol
fidelity (in the sense of Ferrite, Chen/Balzer/Toninho, ECOOP
2022, Theorem 4.3) — if the developer's code type-checks against
`Chan<S, T>`, the resulting protocol follows the structure of S.

pane-session adopts three patterns from **par** (Michal Strba,
https://github.com/faiface/par), a CLL session type library for
Rust: enum-based branching, the Queue streaming combinator, and
the Server connect/suspend/resume lifecycle. The Server pattern
additionally draws from Balzer & Pfenning, "Client-server sessions
in linear logic" (LICS 2021). par is in-process only (values by
move, panics on disconnect); pane-session adapts these patterns
for IPC with serialization, crash safety (Result, not panic), and
transport abstraction.

**The linear discipline**: pane's core subsystems use the subset of
Rust that most closely approximates linear types. Move-only types,
`#[must_use]`, ownership transfer, Drop-based failure compensation.
The developer-facing API is ergonomic; the infrastructure beneath
it is linearly disciplined.

---

## What Is a Pane

A pane is:

1. **Organized state** — body content, tag (title + commands),
   attributes, configuration. Structured through optics.

2. **An interface for views of that state** — visual display,
   filesystem projection at `/pane/`, scripting queries, protocol
   endpoints, remote access. All projections of the same state, kept
   consistent by optic laws.

A pane exists whether or not a compositor is running. A headless
pane has state, has views (filesystem, scripting, protocol), appears
in the namespace — it just doesn't have the display view open.
Display is one view among peers, not the privileged default.

The **compositor** is infrastructure that provides the display view.
It discovers panes that support display handling and projects them
onto the screen. It is not the center of the architecture.

---

## Handler Traits

Two base traits, reflecting that display is a view a pane opts into.
Service traits extend Handler for per-service protocol events.

```rust
/// Every pane implements this. Lifecycle, messaging.
/// The headless-complete interface.
///
/// EAct: the handler store σ for the base protocol.
/// BeOS: BHandler::MessageReceived, split into typed methods.
///
/// `app_message` replaces Be's MessageReceived catch-all for
/// application-defined message types. There is no `fallback` —
/// dispatch is exhaustive.
pub trait Handler: Send + 'static {
    // --- Lifecycle (every pane) ---
    fn ready(&mut self, proxy: &Messenger) -> Result<Flow> {
        Ok(Flow::Continue)
    }
    fn close_requested(&mut self, proxy: &Messenger) -> Result<Flow> {
        Ok(Flow::Stop)
    }
    fn disconnected(&mut self, proxy: &Messenger) -> Result<Flow> {
        Ok(Flow::Stop)
    }

    // --- Messaging (every pane) ---
    fn app_message(&mut self, proxy: &Messenger, msg: Box<dyn Any + Send>) -> Result<Flow> {
        Ok(Flow::Continue)
    }
    fn reply_received(&mut self, proxy: &Messenger, token: u64, payload: Box<dyn Any + Send>) -> Result<Flow> {
        Ok(Flow::Continue)
    }
    fn reply_failed(&mut self, proxy: &Messenger, token: u64) -> Result<Flow> {
        // token: u64 on the wire; handlers use RequestToken<T>
        // for typed correlation (see RequestToken below)
        Ok(Flow::Continue)
    }
    /// Request-reply. The `msg` payload is type-erased (same as
    /// app_message). The `reply` is an obligation — default impl
    /// explicitly drops it, which sends ReplyFailed to the requester.
    /// This is the correct default for panes that don't handle requests.
    /// Overriding handlers MUST either call reply.reply(payload) or
    /// let the ReplyPort drop — there is no third option.
    fn request_received(&mut self, proxy: &Messenger, msg: Box<dyn Any + Send>, reply: ReplyPort) -> Result<Flow> {
        drop(reply); // explicit: sends ReplyFailed
        Ok(Flow::Continue)
    }
    fn pane_exited(&mut self, proxy: &Messenger, pane: Id, reason: ExitReason) -> Result<Flow> {
        Ok(Flow::Continue)
    }
    fn pulse(&mut self, proxy: &Messenger) -> Result<Flow> {
        Ok(Flow::Continue)
    }

    // --- Introspection ---
    fn supported_properties(&self) -> &[PropertyInfo] { &[] }

    // --- Quit (&self for deadlock freedom) ---
    /// &self prevents sending messages during quit negotiation
    /// (no &Messenger parameter). This eliminates the deadlock
    /// vector where quit_requested triggers a message that blocks.
    /// Side effects (save, flush) must happen BEFORE returning true.
    /// If the handler needs to save, it should maintain a dirty flag
    /// and save in close_requested (which has &mut self + &Messenger),
    /// returning Flow::Stop only after the save completes.
    fn quit_requested(&self) -> bool { true }
}

/// Panes that support the display view implement this.
/// The compositor routes input events only to panes whose handler
/// declares display capability.
///
/// Display is a service the pane opts into — DeclareInterest
/// applied to display itself.
///
/// BeOS: BWindow virtual methods (FrameResized, WindowActivated,
/// KeyDown, MouseDown, QuitRequested).
pub trait DisplayHandler: Handler {
    fn display_ready(&mut self, proxy: &Messenger, geom: Geometry) -> Result<Flow> {
        Ok(Flow::Continue)
    }
    fn resized(&mut self, proxy: &Messenger, geom: Geometry) -> Result<Flow> {
        Ok(Flow::Continue)
    }
    fn activated(&mut self, proxy: &Messenger) -> Result<Flow> {
        Ok(Flow::Continue)
    }
    fn deactivated(&mut self, proxy: &Messenger) -> Result<Flow> {
        Ok(Flow::Continue)
    }
    fn key(&mut self, proxy: &Messenger, event: KeyEvent) -> Result<Flow> {
        Ok(Flow::Continue)
    }
    fn mouse(&mut self, proxy: &Messenger, event: MouseEvent) -> Result<Flow> {
        Ok(Flow::Continue)
    }
    fn command_activated(&mut self, proxy: &Messenger) -> Result<Flow> {
        Ok(Flow::Continue)
    }
    fn command_dismissed(&mut self, proxy: &Messenger) -> Result<Flow> {
        Ok(Flow::Continue)
    }
    fn command_executed(&mut self, proxy: &Messenger, cmd: &str, args: &str) -> Result<Flow> {
        Ok(Flow::Continue)
    }
    fn completion_request(&mut self, proxy: &Messenger, input: &str, reply: CompletionReplyPort) -> Result<Flow> {
        Ok(Flow::Continue)
    }
}
```

A headless agent implements `Handler`. A display editor implements
`DisplayHandler`. The compositor discovers display-capable panes
through protocol capability declaration.

### Service traits

Each service protocol has its own handler trait, extending Handler
(not DisplayHandler — services work headless unless they require
display). Service disconnect is scoped to the service trait, not
the base Handler — no growth on Handler per service.

```rust
pub trait ClipboardHandler: Handler {
    fn clipboard_lock_granted(&mut self, proxy: &Messenger, lock: ClipboardWriteLock) -> Result<Flow> {
        Ok(Flow::Continue) // default: drop lock (reverts)
    }
    fn clipboard_lock_denied(&mut self, proxy: &Messenger, clipboard: &str, reason: &str) -> Result<Flow> {
        Ok(Flow::Continue)
    }
    fn clipboard_changed(&mut self, proxy: &Messenger, clipboard: &str, source: Id) -> Result<Flow> {
        Ok(Flow::Continue)
    }
    /// The clipboard service disconnected. Called when the clipboard
    /// calloop channel fires Event::Closed. The handler should
    /// release any assumptions about held resources — outstanding
    /// ClipboardWriteLocks are stale (commit will return Err).
    fn clipboard_service_lost(&mut self, proxy: &Messenger) -> Result<Flow> {
        Ok(Flow::Continue)
    }
}
```

```rust
/// Headless command surface. Registered via open_scripting() +
/// DeclareInterest. A headless pane can execute commands and
/// provide completions without DisplayHandler.
///
/// Display panes that also want scripting implement both
/// DisplayHandler (for input-originated commands) and
/// ScriptingHandler (for script-originated commands).
pub trait ScriptingHandler: Handler {
    fn script_command(&mut self, proxy: &Messenger, cmd: &str, args: &str) -> Result<Flow> {
        Ok(Flow::Continue)
    }
    fn script_completion(&mut self, proxy: &Messenger, input: &str, reply: CompletionReplyPort) -> Result<Flow> {
        Ok(Flow::Continue)
    }
}
```

DnD requires display: `DragHandler: DisplayHandler`.
Other services follow the pattern: `ObserverHandler: Handler`.

### Geometry

```rust
pub struct Geometry {
    /// Logical position (scale-independent).
    pub x: f64,
    pub y: f64,
    /// Logical size (scale-independent).
    pub width: f64,
    pub height: f64,
    /// Physical = Logical × scale_factor.
    pub scale_factor: f32,
}

impl Geometry {
    pub fn physical_size(&self) -> (u32, u32) {
        ((self.width * self.scale_factor as f64) as u32,
         (self.height * self.scale_factor as f64) as u32)
    }
}
```

Logical pixels are the canonical unit. The scale_factor is
provided by the display server for renderers that need physical
coordinates. Headless panes may have virtual geometry (logical
dimensions for layout purposes) or no geometry at all (Handler
has no geometry parameter).

### Flow

```rust
/// Handler control flow. Replaces Result<bool>.
/// Errors are orthogonal — methods return Result<Flow>.
/// EAct §5: actor failure (Err) is separate from session
/// communication (Flow).
///
/// When a handler returns Flow::Stop, the looper exits, drops the
/// handler (triggering Drop compensation on all held obligation
/// handles), then notifies the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Stop,
}
```

---

## Message Types

### The split: values vs obligations

v1's `Message` enum conflates clonable value events with affine
obligation-carrying handles. Four variants panic on Clone. This
violates EAct KP2 (no channel endpoints in message values).

v2 separates them. The public type retains the Be name `Message`
(tier 1 faithful — this is what BMessage was, minus the obligations
that never belonged there).

```rust
/// Value messages — Clone, filter-visible. Pure notifications:
/// something happened, here are the details. No obligations.
///
/// BMessage, corrected: obligations extracted to internal types.
#[derive(Debug, Clone)]
pub enum Message {
    // Lifecycle (no geometry — headless-first)
    Ready,
    CloseRequested,
    Disconnected,
    PaneExited { pane: Id, reason: ExitReason },
    Pulse,
    ReplyFailed { token: u64 },

    // Display (only arrives if DisplayHandler declared)
    DisplayReady(Geometry),
    Resize(Geometry),
    Activated,
    Deactivated,
    Key(KeyEvent),
    Mouse(MouseEvent),
    CommandActivated,
    CommandDismissed,
    CommandExecuted { command: String, args: String },

    // Service notifications (Clone-safe, filter-visible)
    ClipboardLockDenied { clipboard: String, reason: String },
    ClipboardChanged { clipboard: String, source: Id },
}
```

Obligation-carrying messages bypass the filter chain and dispatch
directly to their handler method. They do not form a public enum —
they are internal to the looper.

`Request` bypasses filters as a unit — the inner payload is not
independently filterable. If filters could see it, consuming the
payload would orphan the ReplyPort, violating the linear discipline.

- `CompletionRequest { input, reply: CompletionReplyPort }`
- `AppMessage(Box<dyn Any + Send>)`
- `Reply { token, payload: Box<dyn Any + Send> }`
- `Request(Box<dyn Any + Send>, ReplyPort)`
- `ClipboardLockGranted(ClipboardWriteLock)`

Three categories:

1. **Clone-safe** (Message): freely duplicable, filter-visible.
2. **Move-only** (AppMessage, Reply): non-Clone due to value
   semantics. Dropping is safe — no peer waiting.
3. **Obligation-carrying** (ReplyPort, CompletionReplyPort,
   ClipboardWriteLock): non-Clone due to session obligation.
   Dropping triggers failure compensation via Drop impl.

`AppMessage` is the type-erased escape hatch for worker-to-handler
communication. Obligation handles (ReplyPort, ClipboardWriteLock)
are excluded at compile time via the `AppPayload` marker trait:

```rust
pub trait AppPayload: Clone + Send + 'static {}

pub fn post_app_message<T: AppPayload>(&self, msg: T) -> Result<()>;
```

`AppPayload` requires `Clone + Send + 'static`. All obligation
types (ReplyPort, ClipboardWriteLock, CompletionReplyPort) are
intentionally `!Clone` — they cannot implement `AppPayload` and
cannot be smuggled through AppMessage, even via wrapper structs
(a `struct Smuggle(ReplyPort)` cannot derive Clone). Orphan rules
provide a second layer: external crates cannot impl `AppPayload`
for obligation types. User payloads (strings, structs, enums)
derive Clone trivially. Non-Clone payloads wrap in `Arc`.

A `debug_assert` in the looper dispatch provides defense-in-depth.

---

## Filter Chain

Operates on `Message` only. Never sees obligations.

```rust
pub trait MessageFilter: Send + 'static {
    fn filter(&mut self, msg: &Message) -> FilterAction;
    fn matches(&self, msg: &Message) -> bool { true }
}

pub enum FilterAction {
    /// Pass the message through unchanged. The filter chain
    /// retains ownership and dispatches the original.
    Pass,
    /// Replace the message with a transformed version.
    /// Use case: shortcut filter transforms Key → CommandExecuted.
    Transform(Message),
    /// Consume the message — handler never sees it.
    Consume,
}
```

No panic branches. `Message` derives `Clone` naturally.
`MessageFilter` keeps its Be name (BMessageFilter).

`filter()` takes `&Message` (immutable borrow). On `Pass`, the
chain dispatches the original — no ownership transfer needed. On
`Transform`, the filter provides the replacement. On `Consume`,
the message is dropped. This eliminates the ambiguity of the
previous `Pass(Message)` design where a filter could return a
modified message via Pass.

Filters that transform should preserve variant semantics —
Key→CommandExecuted is valid (same domain: user intent);
Key→CloseRequested is pathological (different domain). The type
system does not enforce this — it is a convention. Debugging tools
log pre-filter and post-filter identity for `Transform` actions.

Filters are applied in registration order. `add_filter` appends
to the chain. Filters can be removed by dropping the returned
`FilterHandle` (which sends `RemoveFilter` to the looper).

---

## Protocol

### Headless-first naming

The server may or may not have a display. The protocol is named
for what it always is, not for one of its modes:

```rust
/// Messages from a pane to the server.
pub enum ClientToServer { ... }

/// Messages from the server to a pane.
pub enum ServerToClient { ... }
```

### Capability declaration

Display capability is declared in the handshake — the server
allocates surface resources based on it. Service capabilities
are declared in the active phase via `DeclareInterest` messages —
services can be opened mid-session.

```rust
pub enum Capability {
    /// Display view — pane can be rendered, accepts input.
    /// Declared in handshake.
    Display,
    /// Clipboard service. Declared via DeclareInterest.
    Clipboard,
    /// Observer (property watching). Declared via DeclareInterest.
    Observer,
    /// Drag and drop (requires Display). Declared via DeclareInterest.
    DragDrop,
    /// Scripting protocol. Declared via DeclareInterest.
    Scripting,
}
```

`DeclareInterest` includes `expected_version: u32` (the version
the client SDK was compiled for). The server accepts if compatible,
or declines with `InterestDeclined { reason: VersionMismatch }`.
This allows services to evolve independently of the base protocol.
Version range negotiation (min/max) deferred to Phase 2 when
server-side version divergence becomes real.

DeclareInterest is irrevocable by default; revocation, if needed,
is an explicit protocol operation with in-flight message cleanup.

### Per-service wire messages

Each service gets its own message types, multiplexed over the
connection with a service discriminant:

```
[length: u32][service: u8][payload: ...]
```

Service 0 = base protocol. Other services assigned at DeclareInterest
time.

Per-service error semantics: a malformed message on service N tears
down service N's channel, not the connection. Connection-level
errors (framing corruption, auth failure) tear down the connection.

### Request cancellation

```rust
// In ClientToServer:
Cancel { token: u64 }
```

Advisory cancellation of in-flight requests (completions, scripting
queries). The server responds with either the original reply (if
already computed) or `ReplyFailed { token }`. Same semantics as
9P's Tflush: the client must handle a reply arriving after
cancellation.

Cancel cancels the *wire request*. Cleaning up handler-side
intermediate state (e.g., clearing `self.clipboard_request`) is
the handler's responsibility.

### Enum-based branching (adapted from par)

pane-session replaces binary `Select<L, R>` / `Branch<L, R>`
nesting with N-ary enum branching via a `SessionEnum` derive macro:

```rust
#[derive(SessionEnum)]
#[repr(u8)]
enum Operation {
    #[session_tag = 0]
    CheckBalance,  // continuation: Send<Amount, End>
    #[session_tag = 1]
    Withdraw,      // continuation: Recv<Amount, Send<Result<Money, Error>, End>>
}
```

The derive generates `choose_*()` methods (sends 1-byte
discriminant, returns typed continuation) and `offer()` (receives
tag, returns enum of dual continuations with exhaustive match).

Wire cost: always 1 byte (vs up to N-1 for binary nesting).
Discriminants are `#[session_tag]`-annotated for wire stability
across versions. Rust's exhaustive match on the offer side
guarantees all branches are handled.

### Streaming (Queue pattern, adapted from par)

Session-typed streaming for observer notifications, clipboard
change streams, filesystem event streams:

```rust
type StreamSend<T, S>;  // push items, then close → S
type StreamRecv<T, S>;  // pop items until Closed → Dual<S>

// Wire: 0x00 + postcard(value) = Item, continue streaming
//       0x01                   = Closed, transition to continuation S
```

The continuation `S` after `Closed` enables clean shutdown
protocols (e.g., send UnsubscribeAck after the last item).

Backpressure: stream items are buffered into the ConnectionSource's
write buffer. If the write buffer hits the high-water mark,
`push()` returns `Err(Backpressure)` — it does NOT block (that
would violate I2). The handler decides: drop the item, stop
producing, or return `Flow::Stop`. OS-level flow control (TCP,
unix socket buffers) provides additional backpressure beneath.

Streams must be closed (send `Closed` on the wire) before session
suspension. A resumed session with a desynchronized stream is a
protocol error.

### Session suspension and resumption (adapted from par)

Server lifecycle for distributed pane:

```
Connect → Active → Suspend → (token held by client) → Resume → Active → Disconnect
```

The server issues a serializable session token on suspend. The
client holds the token across disconnection. On reconnect, the
client presents the token and the server locates the suspended
state. Stateful obligations (locks, pending requests) do NOT
survive suspension — they fail via Drop compensation. The resumed
session re-declares interests, re-negotiates per-service protocol
versions (the server may have upgraded), and re-acquires resources.

Session tokens expire server-side (configurable timeout). If the
server evicts a suspended session (expiry, memory pressure, admin
action), the client discovers at resume time — the server rejects
the token. There is no proactive notification (the client may be
disconnected).

Multi-server suspension is per-Connection, independent. A pane
connected to three servers may have its compositor Connection
suspended while clipboard and registry remain active. "Half-
suspended" is a valid state. Each Connection's suspension is
independent of the others.

Adapted from par's Server/Proxy/Connection module (Michal Strba,
https://github.com/faiface/par). In par, scope isolation (no two
components in the same closure scope) prevents deadlocks. In pane,
process isolation provides the same guarantee. The Server pattern
additionally draws from Balzer & Pfenning, "Client-server sessions
in linear logic" (LICS 2021).

---

## Connection Model

### Multi-server by design

A pane connects to multiple servers. Each server provides different
capabilities. No server is a mandatory intermediary — "host as
contingent server."

A typical topology:
- Compositor on machine B (Display, Input, Layout)
- Clipboard service on machine C (Clipboard)
- Registry on machine D (Roster, AppRegistry)

Each is an independent Connection with its own calloop source.

### App as service router

App holds multiple Connections and routes operations by capability.
The developer sees object APIs (Clipboard, Messenger) that hide
which server backs them. The complexity lives in App, not in
Pane/Looper/Handler.

```rust
impl App {
    /// Connect to the primary server (compositor or headless).
    pub fn connect(signature: &str) -> Result<Self>;

    /// Connect to an additional service server.
    /// The server's capabilities are discovered during handshake.
    pub fn connect_service(&self, addr: impl ToSocketAddrs) -> Result<()>;
}
```

### Service discovery

The App receives a **service map** from the environment — not from
any single server. This is the Plan 9 namespace(1) approach:
declarative configuration, lazy connection.

```
# $PANE_SERVICES or /etc/pane/services.toml
[compositor]
uri = "unix:///run/pane/compositor.sock"

[clipboard]
uri = "tcp://clipboard.internal:9090"
tls = true
```

For headless instances without a compositor, the service map omits
the compositor entry. The App works — it just can't create visual
panes.

### Per-Connection handshake

Every Connection starts with the same Phase 1 (uniform, like 9P's
Tversion):

```
Client → Server: Hello { version, identity: PeerIdentity }
Server → Client: Welcome { version, instance_id, services: Vec<ServiceDecl> }
```

Phase 2 is per-service setup on the same Connection (a server that
provides multiple capabilities handles them on one Connection):

```rust
pub struct ServiceDecl {
    pub kind: Capability,
    pub version: u32,
}
```

The compositor's Welcome says `services: [Display, Input, Layout]`.
The clipboard server says `services: [Clipboard]`. A combined
local server might say `services: [Display, Input, Clipboard]`.

### ConnectionSource

```rust
/// calloop event source for a single Connection.
/// Handles both read (fd-readiness) and write (buffered, flushed
/// on write-readiness). High-water mark backpressure on writes.
pub struct ConnectionSource { ... }
```

Each Connection produces a ConnectionSource. App's dispatcher
routes incoming events from all Connections to the right pane's
looper channel. The looper sees `LooperMessage` variants regardless
of which server they came from.

### Remote connections require TLS

`Connection::remote` requires TLS. PeerIdentity validated against
TLS certificate. Plaintext TCP not supported in production. A
`remote_insecure` may exist for development with explicit opt-in.

### Cross-Connection ordering

Events within a single Connection are FIFO (TCP guarantees this).
Events across Connections are **not causally ordered**. The handler
imposes causal order through its control flow when needed (e.g.,
request clipboard contents *after* receiving paste key event).

The unified batch linearizes events from all sources into a total
order within each dispatch cycle. This gives sequential consistency
per-pane but not causal consistency across servers. The session-type
formalism is silent on cross-session ordering — it's a property
of the physical system, not the protocol.

For cross-Connection patterns (paste: receive key event from
compositor, then fetch clipboard from clipboard service), the
handler is the state machine. The canonical pattern uses
`send_request` + `reply_received`:

```rust
// In command_executed (key event arrives from compositor):
self.clipboard_request = Some(proxy.send_request(
    &clipboard_messenger, ClipboardRead("text/plain"),
)?);

// In reply_received (clipboard data arrives from clipboard server):
if Some(token) == self.clipboard_request.as_ref().map(|t| t.raw()) {
    self.clipboard_request.take();
    let data: Vec<u8> = payload.downcast()?;
    proxy.set_content(&data)?;
}
```

No closure-based sequencing utility. The handler tracks
intermediate state explicitly — this is more debuggable, composes
naturally with Rust's ownership model, and makes the causal
ordering visible in the handler's state machine.

The batch provides a *processing* order, not a *causal* order.
Handlers that need cross-Connection causality must use request/reply,
not event observation.

### Failure isolation

A Connection going down affects only the capabilities it provides.
Other Connections are unaffected. The pane continues running.

- Compositor Connection lost → pane can't display (likely exits)
- Clipboard Connection lost → paste/copy operations return `Err`
- Registry Connection lost → app discovery fails, pane unaffected

This is Plan 9's `Ehangup` applied per-Connection. Failure surfaces
at the use site — when the handler tries to use the lost capability.
Service-specific `*_service_lost()` methods on service traits
provide async notification for handlers holding stale state.

### Scoped pane handles

Each pane gets a handle that can only send messages about itself.
No Id in the public API.

```rust
/// Can only send messages for this pane. The pane ID is baked in.
/// Plan 9 fid principle: the handle IS the name.
pub struct Handle { ... }
```

`Messenger` wraps `Handle` + a `ServiceRouter` that knows which
Connection to use for which operation. Cloneable, Send. Token
allocation for `send_request` managed internally — developers don't
construct raw tokens.

The server MAY enforce per-pane limits on concurrent outstanding
requests to bound obligation growth from Messenger clones.

### RequestToken\<T\>

```rust
/// Typed correlation token for outstanding requests.
/// Wraps u64 for wire compatibility. PhantomData<T> carries the
/// expected reply type — downcast is encapsulated in extract().
#[must_use = "represents an in-flight obligation"]
pub struct RequestToken<T> {
    raw: u64,
    _type: PhantomData<T>,
}

impl<T: Send + 'static> RequestToken<T> {
    pub fn raw(&self) -> u64 { self.raw }
    pub fn extract(&self, payload: Box<dyn Any + Send>) -> Option<T> {
        payload.downcast::<T>().ok().map(|b| *b)
    }
}
```

`send_request<T>` returns `RequestToken<T>`. The handler stores
it and uses `extract()` in `reply_received` for typed correlation:

```rust
fn reply_received(&mut self, proxy: &Messenger, token: u64, payload: Box<dyn Any + Send>) -> Result<Flow> {
    if token == self.pending.raw() {
        if let Some(results) = self.pending.extract(payload) {
            // results: SearchResults — typed, no manual downcast
        }
    }
    Ok(Flow::Continue)
}
```

Tokens are per-Connection (three servers = three namespaces).
Messenger manages token allocation internally.

---

## Service Registration

Services are opened at pane setup time. Opening a service:
1. Resolves the capability to a Connection (via service map)
2. Sends `DeclareInterest` on that Connection
3. Registers a typed calloop source in the looper
4. Returns a handle the developer uses to interact with the service

```rust
// Clipboard registration — the framework finds the right Connection
let clipboard = pane.open_clipboard("system")?;

// In the handler — same as single-server:
fn command_executed(&mut self, proxy: &Messenger, cmd: &str, _: &str) -> Result<Flow> {
    if cmd == "copy" {
        self.clipboard.request_lock()?;
    }
    Ok(Flow::Continue)
}
```

The developer writes zero additional code for multi-server vs
single-server. The difference is App configuration (which servers
to connect to), not handler code.

### Typed ingress, unified batch

Each service's calloop channel carries its own typed event enum.
At the calloop callback boundary, events convert to the looper's
internal representation and enter the unified batch. Total ordering
within the batch is preserved. Coalescing operates within the batch.
Filters see Message (Clone-safe) variants only. Service obligations
bypass filters but remain ordered within the batch.

---

## Dispatch

Match-based dispatch in the looper. The monomorphized match IS
the handler store σ. No explicit Sigma struct — the compiler
provides what it would add.

The looper is generic over `H`:
- `run_with<H: Handler>` for headless panes
- `run_with_display<H: DisplayHandler>` for display panes

Service dispatch uses fn pointers captured at registration time
(the Sigma pattern). When `open_clipboard()` is called on a pane
whose handler implements `ClipboardHandler`, the registration
captures `clipboard_lock_granted`, `clipboard_lock_denied`,
`clipboard_changed`, `clipboard_service_lost` as monomorphized
fn pointers. These are called during batch processing, sharing
`&mut H` with the main dispatch — sequentially, never concurrently
(I7).

---

## The Linear Discipline

### Typestate handles (preserved from v1)

Every obligation-carrying type follows the pattern:

- `#[must_use]` — compiler warns on unused
- Move-only — no Clone
- Single success method consumes — `.commit()`, `.reply()`, `.wait()`
- Drop sends failure terminal — revert, ReplyFailed, RequestClose

Proven on: ReplyPort, CompletionReplyPort, ClipboardWriteLock,
CreateFuture, TimerToken.

### Deeper linearity in v2

- **Service handles**: `open_clipboard()` returns a `ClipboardHandle`
  whose Drop sends `RevokeInterest`. The service lifecycle is
  linearly tracked.
- **Batch processing**: the batch `Vec` is taken (`mem::take`) and
  drained. Events are consumed by dispatch — no re-buffering.
- **Filter chain**: `filter()` takes `Message` by move, returns
  `FilterAction` by move. Each filter consumes and produces.
  Linear transformation chain.
- **Connection**: `ConnectionSource` is move-only. Inserting it
  into calloop consumes it. No aliased access to the fd.

### Where affine falls short

Rust is affine (values can be dropped), not linear (values must
be consumed). The gap is compensated by Drop impls that send
failure terminals. The invariants:

- **I1**: `panic = unwind` in all pane binaries (Drop must fire)
- **I2**: No blocking calls in handler methods (EAct Progress)
- **I3**: Handler callbacks terminate (return Flow)
- **I4**: Typestate handles: `#[must_use]` + Drop compensation
- **I5**: Filters see only Message (Clone-safe, type-enforced)
- **I6**: Sequential single-thread dispatch per pane (BLooper model)
- **I7**: Service dispatch fn pointers called sequentially within
  the batch loop, never concurrently, preserving Rust's exclusive-
  reference invariant on `&mut H`
- **I8**: No blocking cross-service calls within handler methods.
  A handler processing a clipboard event must not call `send_and_wait`
  on the compositor Connection (or vice versa). Preserves DLfActRiS
  strong acyclicity across the multi-server connectivity graph.

### Chan Drop sends ProtocolAbort

`Chan<S, T>` implements `Drop`. If dropped mid-handshake (crash,
early return, panic unwind), Drop sends a `ProtocolAbort` frame
(`[0xFF][0xFF]`) on the transport, then closes it. The peer
receives the abort and frees its session thread immediately —
no TCP timeout wait. Best-effort: if the transport is already
dead (broken pipe), Drop silently succeeds (`let _ = ...`).

### Obligation handle lifetime

Obligation handles (ReplyPort, CompletionReplyPort,
ClipboardWriteLock) are **short-lived**: they should be consumed
within the handler method invocation that receives them. Storing
an obligation handle in `self` defers its resolution until handler
destruction, blocking the requesting pane indefinitely.

This is a convention, not a type-level enforcement. `ReplyPort`
remains `Send` (worker threads may need to reply). A lint that
warns on `self.field = reply` assignments is a future enhancement.
If a handler stores and forgets, the Drop compensation (ReplyFailed)
fires at handler destruction — the linear discipline works, but
the requester waits longer than necessary.

### ClipboardWriteLock::commit

```rust
pub fn commit(self, data: Vec<u8>, metadata: ClipboardMetadata)
    -> Result<(), CommitError>;
```

`commit` takes `self` by value — the lock is consumed. No retry.
`CommitError` variants: `Disconnected` (service gone between lock
grant and commit), `LockRevoked` (server revoked due to timeout
or admin action), `ValidationFailed` (malformed data rejected).
Drop-based Revert fires only if `commit` was never called.

**I8 is enforced at runtime.** The looper maintains a thread-local
`CURRENT_CONNECTION` id during handler dispatch. If `send_and_wait`
is called targeting a Connection other than `CURRENT_CONNECTION`,
the runtime panics in debug builds and logs a warning in release
builds. This turns the advisory invariant into a testable one.
The check cost (one thread-local read) is negligible relative to
the IPC round-trip that `send_and_wait` performs.

**Tokens are per-Connection.** A pane connected to three servers
has three independent token namespaces. `Cancel { token }` is
routed to the Connection that issued the original request. The
Messenger embeds the routing context. Token values may collide
across Connections — this is correct because the namespaces are
disjoint.

### Message enum growth

The Message enum grows with each new Clone-safe notification
variant. This is the same maintenance concern Be had with AppDefs.h
message constants. For service-specific notifications that don't
need filter visibility, consider a `ServiceNotification` wrapper
variant to contain growth rather than flattening everything into
the top-level enum.

### Termination semantics

**`Flow::Stop` (graceful exit):**

1. The looper stops dispatching further events from the batch
2. The handler is dropped — Drop impls fire on all held obligation
   handles (ReplyPort → ReplyFailed, ClipboardWriteLock → Revert)
3. The server is notified via `PaneExited { reason: Graceful }`

**`Err(e)` (crash):**

1. The looper logs the error
2. No further events are dispatched
3. The handler is dropped — same Drop compensation as Flow::Stop
4. The server is notified via `PaneExited { reason: Error(e) }`

Both paths trigger identical Drop compensation. The distinction
is in the exit reason reported to the server and to monitoring
panes (via `pane_exited`). `Flow::Stop` is "I chose to exit."
`Err` is "something went wrong." EAct §5 separates actor failure
(zap/supervision) from session termination (clean end).

Obligation compensation completes before the server is notified.

---

## Developer Experience

### Minimal headless agent

```rust
use pane_app::{Connection, Tag, Handler, Messenger, Flow, Result};

struct StatusAgent;

impl Handler for StatusAgent {
    fn ready(&mut self, proxy: &Messenger) -> Result<Flow> {
        proxy.set_content(b"online")?;
        proxy.set_pulse_rate(Duration::from_secs(60))?;
        Ok(Flow::Continue)
    }

    fn pulse(&mut self, proxy: &Messenger) -> Result<Flow> {
        let status = check_health();
        proxy.set_content(status.as_bytes())?;
        Ok(Flow::Continue)
    }
}

fn main() -> pane_app::Result<()> {
    let (conn, source) = Connection::remote(
        "com.ops.status", "headless.internal:9090",
    )?;
    let pane = conn.create_pane(Tag::new("Server Status"))?.wait()?;
    pane.run_with(source, StatusAgent)
}
```

### Display editor with clipboard

```rust
use pane_app::*;

struct Editor {
    buffer: String,
    clipboard: ClipboardHandle,
}

impl Handler for Editor {
    fn ready(&mut self, proxy: &Messenger) -> Result<Flow> {
        proxy.set_content(self.buffer.as_bytes())?;
        Ok(Flow::Continue)
    }

    fn close_requested(&mut self, _proxy: &Messenger) -> Result<Flow> {
        Ok(Flow::Stop)
    }
}

impl DisplayHandler for Editor {
    fn key(&mut self, proxy: &Messenger, event: KeyEvent) -> Result<Flow> {
        self.buffer.push(event.char);
        proxy.set_content(self.buffer.as_bytes())?;
        Ok(Flow::Continue)
    }

    fn command_executed(&mut self, proxy: &Messenger, cmd: &str, _: &str) -> Result<Flow> {
        if cmd == "copy" {
            self.clipboard.request_lock()?;
        }
        Ok(Flow::Continue)
    }
}

impl ClipboardHandler for Editor {
    fn clipboard_lock_granted(&mut self, _proxy: &Messenger, lock: ClipboardWriteLock) -> Result<Flow> {
        lock.commit(self.buffer.as_bytes().to_vec(), ClipboardMetadata {
            content_type: "text/plain".into(),
            sensitivity: Sensitivity::Normal,
            locality: Locality::Any,
        })?;
        Ok(Flow::Continue)
    }
}

fn main() -> pane_app::Result<()> {
    let (conn, source) = Connection::local("com.pane.editor")?;
    let mut pane = conn.create_pane(
        Tag::new("Editor")
            .command(cmd("copy", "Copy").shortcut("Ctrl+C")),
    )?.wait()?;

    let clipboard = pane.open_clipboard("system")?;
    pane.run_with_display(source, Editor {
        buffer: String::new(),
        clipboard,
    })
}
```

### Closure form (simple case)

```rust
fn main() -> pane_app::Result<()> {
    let (conn, source) = Connection::local("com.example.hello")?;
    let pane = conn.create_pane(Tag::new("Hello"))?.wait()?;
    pane.run(source, |_proxy, msg| match msg {
        Message::CloseRequested => Ok(Flow::Stop),
        _ => Ok(Flow::Continue),
    })
}
```

The handler API is identical for local and remote connections.
Latency differences are observable through timeout behavior and
Result errors, never through different handler methods or Message
variants.

---

## What Is Preserved from v1

- **Session-typed handshake** (pane-session): Chan, Send, Recv,
  Branch, Select, End, Transport trait, finish().
- **Typestate handles**: ReplyPort, CompletionReplyPort,
  ClipboardWriteLock, CreateFuture, TimerToken.
- **calloop event loop**: per-pane, single-threaded, Timer sources.
- **Filter chain**: MessageFilter trait, FilterAction, FilterChain.
  Now operates on obligation-free Message (Clone-safe).
- **Transport layer**: Unix, TCP, TLS, Memory, Proxy, Reconnecting.
- **Optic crate**: Getter/Setter/PartialGetter/PartialSetter,
  FieldLens/FieldAffine/FieldTraversal, composition, laws.
- **Scripting foundation**: PropertyInfo, ScriptableHandler,
  DynOptic, Specifier, AttrValue.
- **Id as UUID**: client-proposed, server-confirmed.
- **Coalescing**: last Resize, last MouseMove within a batch.
- **TimerToken**: cancel-on-drop, dual-path cancellation.

## What Changes from v1

| Component | v1 | v2 |
|---|---|---|
| Handler | 22 methods, monolithic | `Handler` (~11) + `DisplayHandler` (~10) + service traits |
| Message | 19 variants, 4 panic Clone | `Message` (Clone, obligations extracted) + internal obligation types |
| App | One connection, dispatcher thread, pump threads | Multiple Connections + ServiceRouter. calloop read+write per Connection. |
| Messenger | Shared comp_tx, any pane | Scoped handle + ServiceRouter. Routes by capability, not server. |
| Topology | Single server | Multi-server. Per-Connection failure isolation. Service map from environment. |
| Type naming | `PaneId`, `PaneGeometry`, `PaneTitle` | `Id`, `Geometry`, `Title` — crate path is the namespace |
| Protocol names | ClientToComp / CompToClient | ClientToServer / ServerToClient |
| Wire framing | [length][payload] | [length][service][payload] + per-service version negotiation |
| Services | Types exist, no wire protocol | DeclareInterest + per-service wire messages + per-service error isolation |
| Display | Implicit default | Explicit capability (handshake) |
| Capability | Not declared | Handshake for Display, active-phase DeclareInterest for services |
| LooperMessage | 10 variants, growing | Per-protocol typed calloop channels |
| Headless | Added after compositor | The base case |
| Service disconnect | Not handled | Dual: `commit() -> Result` + per-service `*_service_lost()` on service traits |
| Request cancellation | Not supported | `Cancel { token }` (Tflush equivalent) |
| Remote auth | Self-reported identity | TLS required, PeerIdentity validated against certificate |

---

## Resolved Questions

1. **Service trait dispatch**: fn pointers (Sigma pattern). Zero-cost,
   handler type known at registration time. Unanimous.

2. **DeclareInterest placement**: Display in handshake (server
   allocates surface resources). Services in active phase (opened
   mid-session via DeclareInterest message).

3. **ClipboardWriteLock::commit()**: Returns `Result<(), CommitError>`
   where `CommitError` includes `Disconnected` and `LockRevoked`.
   Non-negotiable. Unanimous.

4. **Session resumption**: Deferred implementation. The handshake
   type will need a `Resume` path. DeclareInterest registrations
   survive reconnection; stateful obligations (locks, pending
   requests, open streams) do not — they fail via Drop compensation.
   Streams must be closed before suspension. The reconnecting client
   receives `ready()` again and must re-acquire stateful resources.
   The resume path re-negotiates per-service protocol versions (the
   server may have upgraded during suspension). Obligation-carrying
   messages must not be buffered across reconnection. Tokens expire
   server-side; client discovers expiry at resume time (rejection).
   Multi-server suspension is per-Connection, independent. Reference:
   Plan 9 aan(8), par Server/Proxy/Connection (Michal Strba).

5. **Naming**: `Message` (tier 1 faithful). The type is what BMessage
   was, corrected: obligations extracted to internal types. No
   divergence entry needed.

6. **Service disconnect**: Dual mechanism. `commit() -> Result` for
   synchronous at-call-site detection. `clipboard_service_lost()`
   (and per-service equivalents) on service traits for async
   notification. Scoped to service traits, not base Handler — no
   Handler growth per service.

7. **TLS for remote**: `Connection::remote` requires TLS.
   PeerIdentity validated against TLS certificate. Plaintext TCP
   not supported in production.

8. **Cancel { token }**: Added to ClientToServer. Advisory
   cancellation. Server responds with original reply or ReplyFailed.

9. **Filter modification**: Intentional. `FilterAction` distinguishes
   `Pass` (unchanged) from `Transform` (rewritten) for debuggability.
   Variant-changing transforms are pathological and should be logged.

10. **AppPayload: Clone + Send + 'static**: Obligation handles are
    !Clone, preventing smuggling even via wrapper structs. Orphan
    rules + Clone bound = compile-time enforcement.

11. **Termination**: `Flow::Stop` = graceful. `Err` = crash. Both
    trigger identical Drop compensation. Distinction is in the exit
    reason reported to server and monitors.

12. **Chan Drop sends ProtocolAbort** (`[0xFF][0xFF]`). Peer frees
    session thread immediately on handshake abort.

13. **RequestToken\<T\>**: Typed correlation for send_request. Wraps
    u64 with PhantomData\<T\>. Eliminates downcast boilerplate.
    Tokens per-Connection.

14. **ScriptingHandler: Handler**: Headless command surface via
    DeclareInterest. `command_executed` on DisplayHandler (input)
    and ScriptingHandler (scripting) for their respective sources.

15. **Geometry**: Logical pixels + scale_factor. `physical_size()`
    helper. Resolves HiDPI ambiguity.

16. **I8 runtime enforcement**: Thread-local CURRENT_CONNECTION in
    looper. `send_and_wait` on different Connection: panic (debug),
    log warning (release).

17. **Service map precedence**: `$PANE_SERVICE_OVERRIDES` > manifest
    > `$PANE_SERVICES` > `/etc/pane/services.toml`.

18. **DeclareInterest version**: Single `expected_version` for
    Phase 1. Range negotiation (min/max) deferred to Phase 2.

## Open Questions

1. **Network discovery**: Tier 1 (explicit configuration) is
   specified. Tier 2 (local rendezvous) and Tier 3 (mDNS/DNS-SD)
   are potential future work.

2. **`remote_insecure` escape hatch**: Development workflow for TLS
   requirement. Options: self-signed certs with local CA (no escape
   hatch), or `remote_insecure` with loud warnings.

---

## Implementation Phases

**Phase 1 — Core.** Single-server, headless, no suspension, no
streaming. Validate: Handler/DisplayHandler split, Message/obligation
split, Flow, calloop dispatch, typestate handles, filter chain,
Messenger, Connection, basic protocol (handshake + active phase).
This is the proof that the linear discipline works end-to-end.

**Phase 2 — Distribution.** Multi-server, TLS, service map,
per-Connection tokens, I8 enforcement, DeclareInterest with version
negotiation. Validate: ServiceRouter, per-Connection failure
isolation, cross-Connection ordering, capability declaration.

**Phase 3 — Lifecycle.** Session suspension/resumption, streaming
(Queue pattern), ScriptingHandler. These interact — streams must
close before suspend — so they're designed together.

**Phase 4 — Performance.** Batch coalescing optimizations, write
buffer tuning, connection pooling. Correctness before performance.
