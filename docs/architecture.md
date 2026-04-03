# pane Architecture

A pane is organized state with an interface that allows views of
that state. Display is one view. The namespace (filesystem
projection at `/pane/`, routed queries via optics, remote access)
is another. Both are projections of the same state, structured
by the protocol and kept consistent by optic laws.

pane is a protocol framework that happens to have a display mode,
not a display framework that also works headless. The headless
server — running the same protocol with no display — is the base
case. Display is a capability that panes opt into, not the default.

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

**The functoriality principle**: `Prog(Phase1 + Phase2) ≠
Prog(Phase1) + Prog(Phase2)`. The programs buildable on the full
architecture are not decomposable into programs buildable on each
phase independently. Phase 1 type signatures shape the design
space — developers (including us) build patterns against the
types they see. A Phase 1 type that omits structure needed in
Phase 2 produces patterns that assume that structure doesn't
exist, creating an ecosystem that can't cleanly accommodate the
full design. BeOS demonstrated this: string-based app signatures
(`strcmp()` everywhere) prevented clean evolution to structured
identity when the launch daemon and package management arrived.

Consequence: every type in Phase 1 must be the full architecture's
type, populated minimally. `ServiceId { uuid, name }` from day
one, not `&'static str` that gets promoted later. `ServiceRouter`
with one entry, not a bare sender. The cost is near-zero
(deterministic UUID derivation, HashMap with one entry). The
alternative is a guaranteed future breaking change across every
Protocol impl, Handler, and downstream application.

---

## What Is a Pane

A pane is:

1. **Organized state** — body content, tag (title + commands),
   attributes, configuration. Structured through optics.

2. **An interface for views of that state** — the display view
   (visual projection via the compositor) and the namespace view
   (filesystem projection at `/pane/`, routed queries via optics,
   remote access). Both are projections of the same state,
   structured by the protocol and kept consistent by optic laws.
   The protocol is not itself a view — it governs how views are
   accessed, negotiated, and coordinated.

A pane exists whether or not a compositor is running. A headless
pane has state and appears in the namespace — it simply doesn't
have the display view open. Display is not the default; it is one
view among peers, opted into via capability declaration.

The **compositor** is infrastructure that provides the display view.
It discovers panes that support display handling and projects them
onto the screen. It is not the center of the architecture.

---

## Protocol and Dispatch

### The Protocol trait

Every service relationship in pane — lifecycle, display, clipboard,
routing, application-defined — is a Protocol. The trait links
three things that are otherwise maintained by convention:

```rust
/// Identity of a service in the pane protocol.
///
/// The UUID is the machine identity — a protocol constant,
/// deterministically derived from the name via UUIDv5.
/// Survives renames and travels across federation boundaries
/// where naming conventions may diverge.
/// The name is the human identity — for pane-fs paths, service
/// maps, and logs.
///
/// # Plan 9
///
/// Analogous to qid.path (stable across renames, machine-comparable)
/// alongside the directory entry name (human-chosen, may vary per
/// client's mount point). See qid(5).
pub struct ServiceId {
    pub uuid: Uuid,
    pub name: &'static str,
}

impl ServiceId {
    /// Derive a ServiceId from a reverse-DNS name.
    /// The UUID is deterministically computed via UUIDv5 using
    /// a fixed PANE_NAMESPACE. Zero ceremony — no manual UUID.
    /// Not const fn (UUIDv5 requires SHA-1, not const-evaluable
    /// in the uuid crate). For `const SERVICE_ID` in Protocol
    /// impls, use the `service_id!` proc-macro which computes the
    /// UUID at compile time:
    ///
    /// ```rust
    /// const SERVICE_ID: ServiceId = service_id!("com.pane.clipboard");
    /// // expands to: ServiceId { uuid: Uuid::from_bytes([...]), name: "com.pane.clipboard" }
    /// ```
    ///
    /// This avoids runtime initialization order issues. `new()` is
    /// available for dynamic ServiceId construction (e.g., tests).
    pub fn new(name: &'static str) -> Self {
        ServiceId {
            uuid: Uuid::new_v5(PANE_NAMESPACE, name.as_bytes()),
            name,
        }
    }

    /// Explicit UUID for services that have been renamed but must
    /// keep their wire identity.
    pub fn with_uuid(uuid: Uuid, name: &'static str) -> Self {
        ServiceId { uuid, name }
    }
}

/// A protocol relationship between a pane and a service.
/// Links identity, typed messages into a single type-level definition.
///
/// EAct: formalizes what a session endpoint IS.
/// CLL: the protocol type determines the channel's session type.
/// Plan 9: the typed version of "I have this file in my namespace."
pub trait Protocol {
    /// Service identity (UUID + human-readable name).
    /// The UUID goes on the wire (DeclareInterest).
    /// The name goes in service maps and pane-fs paths.
    const SERVICE_ID: ServiceId;
    /// The typed events this protocol produces.
    /// Serialize + DeserializeOwned because all protocol messages
    /// cross a process boundary (postcard encoding). Even local-only
    /// protocols carry the bound — `#[derive(Serialize)]` is trivial,
    /// and the bound prevents accidentally introducing a protocol
    /// that works in-process but fails on the wire.
    type Message: Serialize + DeserializeOwned + Send + 'static;
}
```

Keep Protocol minimal: SERVICE_ID + Message. Do not accumulate
associated types for caching, reconnection, priority — those
belong on the service implementation.

**Naming convention** (codify now, per Be's lesson with
inconsistent `application/x-vnd.*` signatures): service names
use reverse-DNS notation. Framework services: `com.pane.*`.
Third-party: `com.vendor.*` or `org.project.*`. Application-
local protocols: `com.vendor.app.*`. The convention is part of
the ServiceId contract, not an afterthought.

### Handles\<P\>: the uniform dispatch trait

```rust
/// A handler that can receive messages from protocol P.
/// Each impl is one entry in EAct's handler store σ.
///
/// The looper dispatches P::Message to this method via a
/// monomorphized fn pointer captured at service registration.
pub trait Handles<P: Protocol> {
    fn receive(&mut self, proxy: &Messenger, msg: P::Message) -> Result<Flow>;
}
```

The type system enforces: if a pane declares interest in a
protocol, its handler must implement `Handles<P>` for that
protocol. Registration (`open_clipboard()`) requires the bound
`H: Handles<Clipboard>` at compile time.

### Named methods via attribute macro

The developer writes named functions. A `#[pane::protocol_handler]`
attribute macro on the `impl` block generates the
`Handles<P>::receive` match that delegates to them. This is
BWindow::DispatchMessage translated to Rust — framework-generated
dispatch calling developer-provided hooks.

(`#[derive(...)]` in Rust applies only to type definitions, not
impl blocks. This is a procedural attribute macro, not a derive.)

```rust
#[pane::protocol_handler(Clipboard)]
impl Editor {
    fn lock_granted(&mut self, proxy: &Messenger, lock: ClipboardWriteLock) -> Result<Flow> {
        lock.commit(self.buffer.as_bytes().to_vec(), metadata)?;
        Ok(Flow::Continue)
    }
    fn lock_denied(&mut self, proxy: &Messenger, clipboard: &str, reason: &str) -> Result<Flow> {
        Ok(Flow::Continue)
    }
    fn changed(&mut self, proxy: &Messenger, clipboard: &str, source: Id) -> Result<Flow> {
        Ok(Flow::Continue)
    }
    fn service_lost(&mut self, proxy: &Messenger) -> Result<Flow> {
        Ok(Flow::Continue)
    }
}

// The macro generates:
impl Handles<Clipboard> for Editor {
    fn receive(&mut self, proxy: &Messenger, msg: ClipboardMessage) -> Result<Flow> {
        match msg {
            ClipboardMessage::LockGranted(lock) => self.lock_granted(proxy, lock),
            ClipboardMessage::LockDenied { clipboard, reason } =>
                self.lock_denied(proxy, &clipboard, &reason),
            ClipboardMessage::Changed { clipboard, source } =>
                self.changed(proxy, &clipboard, source),
            ClipboardMessage::ServiceLost =>
                self.service_lost(proxy),
        }
    }
}
```

The macro must be transparent: `cargo expand` produces a match
a human could write in 30 seconds. No runtime indirection. The
macro generates the match arms; Rust's exhaustive match check IS
the exhaustiveness guarantee. If a handler method is missing for
a variant, `rustc` emits a non-exhaustive pattern error. The macro
does not discover variants — it generates code that fails to
compile if the handler is incomplete. Variant-to-method mapping
uses `#[handles(VariantName)]` attributes or snake_case convention.

Note: `#[derive(ProtocolHandler)]` appears in some examples as
shorthand. The actual mechanism is the attribute macro
`#[pane::protocol_handler(P)]` on the impl block.

### Framework protocols

Lifecycle, display, and messaging are protocols with the same
structure. The base `Handler` trait provides named methods for
lifecycle + messaging (the protocols every pane speaks). Display
is a separate protocol that panes opt into.

```rust
/// Lifecycle protocol — every pane. Part of the base protocol
/// Part of the Control protocol (wire service 0, implicit,
/// never DeclareInterest'd).
struct Lifecycle;
impl Protocol for Lifecycle {
    const SERVICE_ID: ServiceId = ServiceId::new("com.pane.lifecycle");
    type Message = LifecycleMessage;
}

/// Display protocol — panes with a visual surface. Also part of
/// the base protocol (declared in handshake, not via DeclareInterest).
struct Display;
impl Protocol for Display {
    const SERVICE_ID: ServiceId = ServiceId::new("com.pane.display");
    type Message = DisplayMessage;
}
```

Lifecycle and Display share the **Control** protocol (wire
service 0, implicit). They are bundled in `ControlMessage` — a
single enum containing lifecycle, display, and connection-
management variants (DeclareInterest, ServiceTeardown). This is
9P's single-connection multiplexing.

The Control protocol is never negotiated — it exists by virtue
of having a connection. It is the control plane for the session.

**Dispatch path for Control protocol variants:** the looper
receives `ControlMessage` from wire service 0, pattern-matches
on the variant, and routes: lifecycle variants dispatch to
`Handler` methods, display variants dispatch to `DisplayHandler`
methods, connection-management variants (`DeclareInterest`,
`ServiceTeardown`) are handled internally by the framework.
Display capability is declared in the handshake (Hello's
interests list), not via DeclareInterest — the server allocates
surface resources at connection time. The developer never sees
`ControlMessage` directly.

Services with their own negotiated wire discriminants (clipboard,
routing) get their session-local u8 from the server during
DeclareInterest.

The `Handler` trait provides named methods for lifecycle and
messaging. These are not separate Protocols — they are universal
capabilities every pane has by virtue of existing. Messaging
methods (request_received, pane_exited, pulse) are on Handler
directly because there is no DeclareInterest for messaging:

```rust
/// Every pane implements this. Lifecycle + messaging.
/// The headless-complete interface.
///
/// Named methods are the developer-facing API. The looper
/// dispatches lifecycle events through Handles<Lifecycle>;
/// messaging methods are on Handler directly (universal,
/// no DeclareInterest).
pub trait Handler: Send + 'static {
    fn ready(&mut self, proxy: &Messenger) -> Result<Flow> { Ok(Flow::Continue) }
    fn close_requested(&mut self, proxy: &Messenger) -> Result<Flow> { Ok(Flow::Stop) }
    fn disconnected(&mut self, proxy: &Messenger) -> Result<Flow> { Ok(Flow::Stop) }
    fn pulse(&mut self, proxy: &Messenger) -> Result<Flow> { Ok(Flow::Continue) }
    /// Incoming request from another pane. The payload is intentionally
    /// type-erased: the requester and receiver may have different types
    /// (different processes, different T). The service_id identifies the
    /// protocol the sender used — the receiver checks it before
    /// downcasting (analogous to BMessage's `what` field). Protocol-
    /// defined requests route through Handles<P> with typed messages
    /// instead. The reply is an obligation — default drops it (sends
    /// ReplyFailed).
    fn request_received(&mut self, proxy: &Messenger, service: ServiceId, msg: Box<dyn Any + Send>, reply: ReplyPort) -> Result<Flow> {
        drop(reply);
        Ok(Flow::Continue)
    }
    // reply_received and reply_failed are NOT on Handler.
    // Replies route to per-request Dispatch entries (see Request/Reply).
    fn pane_exited(&mut self, proxy: &Messenger, pane: Id, reason: ExitReason) -> Result<Flow> { Ok(Flow::Continue) }
    fn supported_properties(&self) -> &[PropertyInfo] { &[] }
    /// &self for deadlock freedom. Side effects must happen
    /// before returning true (save in close_requested, not here).
    fn quit_requested(&self) -> bool { true }
}

/// Display protocol — panes with a visual surface.
pub trait DisplayHandler: Handler {
    fn display_ready(&mut self, proxy: &Messenger, geom: Geometry) -> Result<Flow> { Ok(Flow::Continue) }
    fn resized(&mut self, proxy: &Messenger, geom: Geometry) -> Result<Flow> { Ok(Flow::Continue) }
    fn activated(&mut self, proxy: &Messenger) -> Result<Flow> { Ok(Flow::Continue) }
    fn deactivated(&mut self, proxy: &Messenger) -> Result<Flow> { Ok(Flow::Continue) }
    fn key(&mut self, proxy: &Messenger, event: KeyEvent) -> Result<Flow> { Ok(Flow::Continue) }
    fn mouse(&mut self, proxy: &Messenger, event: MouseEvent) -> Result<Flow> { Ok(Flow::Continue) }
    fn command_activated(&mut self, proxy: &Messenger) -> Result<Flow> { Ok(Flow::Continue) }
    fn command_dismissed(&mut self, proxy: &Messenger) -> Result<Flow> { Ok(Flow::Continue) }
    fn command_executed(&mut self, proxy: &Messenger, cmd: &str, args: &str) -> Result<Flow> { Ok(Flow::Continue) }
    fn completion_request(&mut self, proxy: &Messenger, input: &str, reply: CompletionReplyPort) -> Result<Flow> { Ok(Flow::Continue) }
}
```

```rust
/// Routing protocol — panes with a namespace projection.
/// The headless equivalent of DisplayHandler: Display projects
/// state visually, Routing projects state as structured data
/// accessible through pane-fs queries and remote commands.
///
/// Declared via DeclareInterest (not handshake — unlike Display,
/// routing capability is opened mid-session when the handler is
/// ready to serve queries).
///
/// Phase 3 implementation. Trait defined here for completeness.
pub trait RoutingHandler: Handler {
    fn route_query(&mut self, proxy: &Messenger, query: RouteQuery, reply: ReplyPort) -> Result<Flow>;
    fn route_command(&mut self, proxy: &Messenger, cmd: &str, args: &str) -> Result<Flow> { Ok(Flow::Continue) }
}
```

Handler, DisplayHandler, and RoutingHandler are special cases of
Protocol + Handles\<P\> with pre-defined named methods. They exist
for ergonomics — the lifecycle, display, and routing protocols
benefit from named-method discoverability. Display is the visual
projection of pane state; Routing is the namespace projection.
Both are opt-in capabilities on top of Handler.

### Service protocols

Clipboard, routing, observer, DnD — each defines a Protocol
and the developer implements Handles\<P\> via the derive macro.
The protocol's Message enum defines the variants; the macro
generates the dispatch; the developer provides named handlers.

```rust
struct Clipboard;
impl Protocol for Clipboard {
    const SERVICE_ID: ServiceId = ServiceId::new("com.pane.clipboard");
    type Message = ClipboardMessage;
}

pub enum ClipboardMessage {
    LockGranted(ClipboardWriteLock),
    LockDenied { clipboard: String, reason: String },
    Changed { clipboard: String, source: Id },
    ServiceLost,
}

struct Routing;
impl Protocol for Routing {
    const SERVICE_ID: ServiceId = ServiceId::new("com.pane.routing");
    type Message = RoutingMessage;
}
```

DnD requires display: `open_drag_drop()` requires
`H: Handles<Display> + Handles<DragDrop>`.

### Pointer policy (mouse coalescing opt-out)

```rust
/// Mouse event delivery policy for the display view.
/// BeOS: B_NO_POINTER_HISTORY / B_FULL_POINTER_HISTORY per-view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointerPolicy {
    /// Deliver only the most recent MouseMove per batch.
    #[default]
    Coalesce,
    /// Deliver every MouseMove event. Required for drawing
    /// applications, gesture recognition, ink input.
    FullHistory,
}

impl Messenger {
    pub fn set_pointer_policy(&self, policy: PointerPolicy) -> Result<()>;
}
```

Per-pane, set in `display_ready` or dynamically. Server-side
filtering — FullHistory tells the server to send all events;
Coalesce tells it to batch. Don't send events over the wire
just to drop them client-side.

### Application-defined protocols

Applications define their own Protocol for custom messages.
`Message<T>` with `What(T)` is the application protocol:

```rust
struct EditorProtocol;
impl Protocol for EditorProtocol {
    const SERVICE_ID: ServiceId = ServiceId::new("com.example.editor");
    type Message = EditorMessage;
    // Local only — never DeclareInterest'd, never on the wire.
    // The UUID exists for identification; routing is looper-local.
}

enum EditorMessage {
    SearchResult(Vec<Match>),
    SpellCheckComplete { corrections: Vec<Correction> },
    AutoSaveFinished,
}

#[pane::protocol_handler(EditorProtocol)]
impl Editor {
    fn search_result(&mut self, proxy: &Messenger, matches: Vec<Match>) -> Result<Flow> { ... }
    fn spell_check_complete(&mut self, proxy: &Messenger, corrections: Vec<Correction>) -> Result<Flow> { ... }
    fn auto_save_finished(&mut self, proxy: &Messenger) -> Result<Flow> { ... }
}
```

Application messages are `What(T)` in `Message<T>` — local to the
looper, never cross the wire, typed at compile time. This replaces
`app_message(Box<dyn Any + Send>)` with exhaustive typed dispatch.

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
    /// f32: Wayland's wl_output scale is fixed-point that fits in
    /// f32; f64 would imply false precision. Logical coordinates
    /// use f64 for sub-pixel precision in layout math.
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

Clonable value events and affine obligation-carrying handles must
not share a type. Conflating them produces Clone panics and
violates EAct KP2 (no channel endpoints in message values).

The public type retains the Be name `Message` (tier 1 faithful —
this is what BMessage was, minus the obligations that never
belonged there).

```rust
/// Value messages — Clone, filter-visible. Pure notifications:
/// something happened, here are the details. No obligations.
///
/// BMessage, corrected: obligations extracted to internal types.
///
/// This enum covers the base protocol only (lifecycle + display).
/// Service events (clipboard, observer, drag-drop) are NOT in this
/// enum — they dispatch through Handles<P>::receive with their own
/// per-protocol message types. This keeps Message closed: adding a
/// new framework service does not modify this enum and does not
/// break existing exhaustive matches.
#[derive(Debug, Clone)]
pub enum Message {
    // Lifecycle (no geometry — headless-first)
    Ready,
    CloseRequested,
    Disconnected,
    PaneExited { pane: Id, reason: ExitReason },
    Pulse,

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
}
```

Service events are delivered through `Handles<P>::receive`, not
through the `Message` enum. `ClipboardMessage` (including both
Clone-safe notifications like `Changed` and obligation-carrying
handles like `LockGranted`) is the protocol's own message type:

```rust
pub enum ClipboardMessage {
    LockGranted(ClipboardWriteLock),
    LockDenied { clipboard: String, reason: String },
    Changed { clipboard: String, source: Id },
    ServiceLost,
}
```

Filter visibility for service events: filters that need to observe
service notifications register per-service filter hooks at service
registration time (e.g., `open_clipboard()` installs the clipboard
filter hook). These hooks are keyed by `ServiceId` and can inspect
the protocol's Clone-safe message variants. The base filter chain
(operating on `Message`) never sees service events.

Obligation-carrying messages bypass the filter chain and dispatch
directly to their handler method. They do not form a public enum —
they are internal to the looper.

`Request` bypasses filters as a unit — the inner payload is not
independently filterable. If filters could see it, consuming the
payload would orphan the ReplyPort, violating the linear discipline.

- `CompletionRequest { input, reply: CompletionReplyPort }`
- `Request(Box<dyn Any + Send>, ReplyPort)`
- `ClipboardLockGranted(ClipboardWriteLock)`
- `Reply { token, payload }` — internal, routes to Dispatch entries
- `ReplyFailed { token }` — internal, routes to Dispatch entries

Reply and ReplyFailed never surface to the handler. They route
to per-request Dispatch entries (see Request/Reply below).

Two categories visible to the handler:

1. **Clone-safe** (Message): freely duplicable, filter-visible.
2. **Obligation-carrying** (ReplyPort, CompletionReplyPort,
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

**When to use `post_app_message` vs application-defined protocols:**
Application protocols (via `Handles<P>` + derive macro) are for
structured, exhaustively-checked dispatch from worker threads. Use
them when the message vocabulary is known at compile time.
`post_app_message` (via `AppPayload`) is for simple fire-and-forget
notifications that don't warrant a full Protocol definition. When
in doubt, use a Protocol — exhaustive matching catches more bugs
than downcast.

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
the message is dropped. This eliminates the ambiguity of a design where `Pass` carries
the message — a filter could silently return a modified message
via Pass, conflating pass-through with transformation.

Filters that transform should preserve variant semantics —
Key→CommandExecuted is valid (same domain: user intent);
Key→CloseRequested is pathological (different domain). The type
system does not enforce this — it is a convention. Debugging tools
log pre-filter and post-filter identity for `Transform` actions.

Filters run before handler dispatch. If a filter returns `Consume`,
the handler method does not fire for that message. If a filter
returns `Transform(msg)`, the handler receives the transformed
message. The filter chain is the first stage; handler dispatch is
the second.

Filters are applied in registration order. `add_filter` appends
to the chain. Filters can be removed by dropping the returned
`FilterHandle` (which sends `RemoveFilter` to the looper).

Filter ordering matters: earlier filters see original messages;
later filters see the results of earlier transformations. If
filter A transforms Key → CommandExecuted, filter B (registered
after A) sees CommandExecuted, not the original Key. This is
correct for composition — a shortcut filter registered first
transforms key combos, and a command-logging filter registered
second observes the resulting commands.

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

Services are identified by globally unique names (reverse-DNS).
Display is declared in the handshake. Other services are declared
via `DeclareInterest` in the active phase. The server assigns a
compact session-local wire discriminant (u8) for each accepted
service.

**Initial binding (handshake).** Hello lists requested services.
Welcome returns bindings:

```rust
pub struct ServiceBinding {
    pub service: ServiceId,     // UUID + name
    pub session_id: u8,         // wire discriminant for this connection
    pub version: u32,           // negotiated version
}

// In Welcome:
pub services: Vec<ServiceBinding>,
```

**Late binding (active phase).** Services opened mid-session:

```rust
// In ClientToServer:
DeclareInterest {
    service: ServiceId,         // UUID + name
    expected_version: u32,
}

// In ServerToClient:
InterestAccepted {
    service_uuid: Uuid,         // echo UUID (client knows the name)
    session_id: u8,             // wire discriminant assigned by server
    version: u32,
}
InterestDeclined {
    service_uuid: Uuid,
    reason: DeclineReason,      // VersionMismatch, ServiceUnknown
}
```

One round-trip per late-binding service. Initial services are
batched in the handshake (zero additional round-trips). The server
accepts or declines; `InterestDeclined::ServiceUnknown` for
services the server doesn't provide.

The session-local u8 is a per-connection fid (Plan 9 lineage).
Different connections may assign different u8 values to the same
service. The 256-slot ceiling is per-connection (no connection
needs 256 simultaneous services).

DeclareInterest is revoked when the service handle is dropped
(sends `RevokeInterest`). In-flight messages for the revoked
service are discarded by the looper. RevokeInterest is best-effort
(`let _ = ...`) — if the Connection is dead, the server already
knows.

The SDK provides a short form: `"clipboard"` expands to
`"com.pane.clipboard"` for framework services.

### Per-service wire messages

Each service gets its own message types, multiplexed over the
connection with a session-local discriminant:

```
[length: u32][service: u8][payload: ...]
```

Wire service 0 = the Control protocol (implicit, not negotiated).
The `ControlMessage` enum contains lifecycle, display, and
connection-management variants (DeclareInterest, ServiceTeardown).
The deserializer knows the type because the Control enum is fixed.

Other service discriminants are assigned by the server during
DeclareInterest (initial binding in handshake, or late binding in
active phase). The mapping `"com.pane.clipboard" → u8:3` is per-
connection. The `ServiceRouter` maintains this mapping.

Per-service error semantics: a malformed message on service N tears
down service N's channel, not the connection. The server signals
teardown via `ServiceTeardown { service: u8, reason }` on the base
Control protocol (wire service 0). This triggers the service-specific
`*_service_lost()` callback. Connection-level errors (framing
corruption, auth failure) tear down the connection.

### Request cancellation

```rust
// In ClientToServer:
Cancel { token: u64 }
```

Advisory cancellation of in-flight requests (completions, routing
queries). The server responds with either the original reply (if
already computed) or `ReplyFailed { token }`. Same semantics as
9P's Tflush: the client must handle a reply arriving after
cancellation.

Cancel cancels the *wire request*. With Dispatch entries, handler-side
cleanup is automatic — the Dispatch entry is removed by CancelHandle.
No ghost state to clear.

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
Client → Server: Hello { version, max_message_size: u32, interests: Vec<ServiceInterest> }
Server → Client: Welcome { version, instance_id, max_message_size: u32, bindings: Vec<ServiceBinding> }
```

Hello declares requested services. Welcome returns bindings (name
→ session-local wire ID + negotiated version). PeerAuth is derived
from the transport (SO_PEERCRED for unix, TLS certificate for
remote) — not carried in Hello.

```rust
pub struct ServiceInterest {
    pub service: ServiceId,     // UUID + name
    pub expected_version: u32,
}

pub struct ServiceBinding {
    pub service: ServiceId,     // UUID + name
    pub session_id: u8,         // wire discriminant for this connection
    pub version: u32,           // negotiated version
}
```

The compositor's Welcome says `bindings: [{com.pane.display, 0, 1}]`.
The clipboard server says `bindings: [{com.pane.clipboard, 1, 1}]`.
A combined local server might bind both on one connection.

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

`Connection::remote` requires TLS. `PeerAuth` is derived from the
transport: `PeerAuth::Kernel { uid, pid }` for unix sockets (via
SO_PEERCRED), `PeerAuth::Certificate { subject, issuer }` for TLS.
The Hello message carries no identity — authentication is transport-
level. `pane_owned_by()` checks `PeerAuth`, not self-reported
strings. Plaintext TCP is not supported. No `remote_insecure`
escape hatch — ship `pane dev-certs` tooling for development
(see resolved question 25).

### Maximum message size

The Hello/Welcome exchange negotiates `max_message_size`. Both
sides send their maximum; the effective limit is the minimum.
Default: 16MB. The server rejects frames exceeding this. The
`length` field in `[length: u32][service: u8][payload]` counts
the service byte plus payload (total frame = 4-byte length field +
service byte + payload bytes). The length field does NOT include
itself.

### Serialization format

All payloads use **postcard** encoding (the same format used by
pane-session for the handshake). This is the canonical wire format
for all active-phase messages.

### Cross-Connection ordering

Events within a single Connection are FIFO (TCP guarantees this).
Events across Connections are **not causally ordered**. The handler
imposes causal order through its control flow when needed (e.g.,
request clipboard contents *after* receiving paste key event).

The unified batch linearizes events from all sources into a total
order within each dispatch cycle. This gives sequential consistency
per-pane but not causal consistency across servers. Events from
different Connections that appear in the same batch have an arbitrary
relative order determined by read readiness, not by any happened-
before relation between the sending servers. The session-type
formalism is silent on cross-session ordering — it's a property
of the physical system, not the protocol.

For cross-Connection patterns (paste: receive key event from
compositor, then fetch clipboard from clipboard service), the
handler uses `send_request` with typed callbacks:

```rust
fn command_executed(&mut self, proxy: &Messenger, cmd: &str, _: &str) -> Result<Flow> {
    if cmd == "paste" {
        proxy.send_request::<Self, ClipboardData>(
            &self.clipboard_messenger,
            ClipboardRead("text/plain"),
            |editor, proxy, data| {
                editor.insert_text(&data.content);
                proxy.set_content(editor.buffer.as_bytes())?;
                Ok(Flow::Continue)
            },
            |editor, proxy| {
                editor.show_status("Paste failed");
                Ok(Flow::Continue)
            },
        )?;
    }
    Ok(Flow::Continue)
}
```

No ghost state. No manual token correlation. The callback receives
the typed reply directly. The causal ordering (key event → clipboard
fetch → insert) is expressed in the callback chain, not in handler
state fields.

The batch provides a *processing* order, not a *causal* order.
Handlers that need cross-Connection causality use `send_request`
callbacks, not event observation.

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

### Request/Reply via the Dispatch

Request/reply is modeled as per-request Dispatch entries, not ghost
state in the handler. Each `send_request` creates a one-shot typed
dispatch slot in the looper's `Dispatch`.

```rust
impl Messenger {
    /// Send a request and register a typed reply callback.
    ///
    /// EAct: E-Suspend installs a one-shot handler in σ for
    /// session type Recv<RequestReplyMessage<R>, End>.
    /// The entry is consumed on reply (E-React) or failure.
    ///
    /// Returns a CancelHandle for optional cancellation.
    pub fn send_request<H, R>(
        &self,
        target: &Messenger,
        msg: impl Serialize + Send + 'static,
        on_reply: impl FnOnce(&mut H, &Messenger, R) -> Result<Flow> + Send + 'static,
        on_failed: impl FnOnce(&mut H, &Messenger) -> Result<Flow> + Send + 'static,
    ) -> Result<CancelHandle>
    where H: Handler + 'static, R: DeserializeOwned + Send + 'static;
}

/// Handle for cancelling an outstanding request.
/// Drop does nothing — the request completes normally.
/// .cancel(self) removes the Dispatch entry without firing callbacks.
/// Inverted from ReplyPort: drop = happy path, cancel = voluntary abort.
pub struct CancelHandle {
    /// Which Connection to send Cancel on.
    connection_id: ConnectionId,
    /// The request token within that Connection's namespace.
    token: u64,
    /// Channel to the looper for Dispatch entry removal.
    looper_tx: LooperSender,
}

impl CancelHandle {
    /// Cancel the request. Consumes self. Late replies silently dropped.
    pub fn cancel(self) { ... }
}
// Drop: intentionally no-op. Uncancelled request completes normally.
```

No ghost state in the handler. The correlation between request and
reply is structural — guaranteed by the Dispatch entry, not by manual
matching. Multiple requests may be outstanding simultaneously,
including to the same target — each gets an independent Dispatch
entry with a unique token (S1).

**Callback capture.** Callbacks are `FnOnce + Send + 'static` —
they cannot capture borrows from the calling scope. This is
correct: the callback outlives the handler invocation (EAct
E-Suspend: Dispatch entries persist across idle). The `&mut H` first
parameter provides handler state at reply time:

```rust
// Pattern 1: Handler state via &mut H (covers ~90% of cases)
proxy.send_request::<Self, SearchResults>(
    &target, query,
    |editor, proxy, results| {
        // editor IS &mut Self — full handler access
        editor.display_results(&results);
        Ok(Flow::Continue)
    },
    |editor, _| { editor.show_status("failed"); Ok(Flow::Continue) },
)?;

// Pattern 2: Capture owned context from the call site
let term = query.to_owned();
proxy.send_request::<Self, SearchResults>(
    &target, SearchQuery(term.clone()),
    move |handler, proxy, results| {
        handler.display_results(&term, results);
        Ok(Flow::Continue)
    },
    |handler, _| { handler.show_status("failed"); Ok(Flow::Continue) },
)?;
```

The callback receives `&mut H` (handler state) and the typed
reply `R` directly. No `Box<dyn Any>` at the handler surface.
The framework handles the type boundary: for same-process requests,
a downcast (`Box<dyn Any + Send>` → `R`); for cross-process
requests, deserialization (postcard → `R`, where `R: DeserializeOwned`).
Both are internal to the framework — the developer sees typed `R`.

**On disconnect**: `dispatch.fail_connection()` fires `on_failed` for every
pending entry before `handler.disconnected()` is called. The
handler gets per-request failure notification.

**On cancellation**: `cancel_handle.cancel()` removes the Dispatch
entry without firing callbacks. Late-arriving replies are silently
dropped. Same as 9P's Tflush. Dropping the CancelHandle without
calling cancel is a no-op — the request completes normally.

**Lifecycle** (EAct terms):
- **E-Suspend**: `send_request` installs entry in σ
- **Active**: entry waits; looper services other sessions
- **E-React**: reply arrives, entry consumed, callback fires
- **Failed**: target drops ReplyPort → `on_failed` fires
- **Cancelled**: handler-initiated removal, no callbacks
- **Abandoned**: handler drops (Flow::Stop, panic) → entry dropped
  without callbacks. Safe: pending entry is a receive-endpoint;
  dropping it doesn't block any peer (DLfActRiS §3.2).

**What this removes from Handler**: `reply_received(token, payload)`
and `reply_failed(token)` are gone. Reply dispatch is structural,
not a handler method. `request_received` stays (the server side
of request/reply, receiving from other panes).

Tokens are per-Connection (three servers = three namespaces).
Token allocation is internal to the framework.

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

Service calloop sources registered during handler methods take
effect on the next dispatch cycle, not the current batch. Events
from a newly registered source cannot arrive in the same batch
as the registration.

### Typed ingress, unified batch

Each service's calloop channel carries its own typed event enum.
At the calloop callback boundary, events convert to the looper's
internal representation and enter the unified batch. Total ordering
within the batch is preserved. Coalescing operates within the batch.
The base filter chain sees `Message` (Clone-safe) variants only.
Service events dispatch through `Handles<P>` and are not visible
to the base filter chain. Per-service filter hooks, registered
at service open time, can observe Clone-safe service events.
Obligation-carrying messages bypass all filters and remain ordered
within the batch.

---

## Dispatch

Match-based dispatch in the looper. The monomorphized match IS
the handler store σ. No explicit struct — the compiler
provides what it would add.

The looper is generic over `H`:
- `run_with<H: Handler>` for headless panes
- `run_with_display<H: DisplayHandler>` for display panes

Service dispatch uses fn pointers captured at registration time
(the Dispatch pattern). When `open_clipboard()` is called on a pane
whose handler implements `Handles<Clipboard>`, the registration
captures the monomorphized `Handles<Clipboard>::receive` as a fn
pointer. The derive macro's generated match delegates to the
developer's named methods. These are called during batch
processing, sharing `&mut H` with the main dispatch — sequentially,
never concurrently (I7).

The closure form (`pane.run(source, |proxy, msg| ...)`) receives
`Message` (Clone-safe events) only. It cannot handle obligation-
carrying protocols (clipboard locks, completion requests, incoming
requests with ReplyPort). These require the struct + trait form
(`pane.run_with(source, handler)`).

---

## The Linear Discipline

### Typestate handles

Every obligation-carrying type follows the pattern:

- `#[must_use]` — compiler warns on unused
- Move-only — no Clone
- Single success method consumes — `.commit()`, `.reply()`, `.wait()`
- Drop sends failure terminal — revert, ReplyFailed, RequestClose

Proven on: ReplyPort, CompletionReplyPort, ClipboardWriteLock,
CreateFuture, TimerToken, ClipboardHandle (Drop sends
RevokeInterest).

Destruction sequence on handler exit: dispatch.fail_connection() (per S4)
→ handler dropped → service handles (ClipboardHandle, etc.)
dropped during handler field destruction → Drop sends
RevokeInterest (best-effort, `let _ = ...`).

### Extended linear discipline

- **Service handles**: `open_clipboard()` returns a `ClipboardHandle`
  whose Drop sends `RevokeInterest`. The service lifecycle is
  linearly tracked.
- **Batch processing**: the batch `Vec` is taken (`mem::take`) and
  drained. Events are consumed by dispatch — no re-buffering.
- **Filter chain**: `filter()` takes `&Message` (immutable borrow).
  Returns `FilterAction`: `Pass` (no-op), `Transform(Message)`
  (replacement), or `Consume` (drop). The chain retains ownership
  on Pass; only Transform produces a new value.
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
- **I5**: Base filters see only Message (Clone-safe, type-enforced);
  per-service filter hooks see Clone-safe service events
- **I6**: Sequential single-thread dispatch per pane (BLooper model)
- **I7**: Service dispatch fn pointers called sequentially within
  the batch loop, never concurrently, preserving Rust's exclusive-
  reference invariant on `&mut H`
- **I8**: No blocking calls (`send_and_wait`) in handler methods.
  Prevents cross-Connection blocking within handlers. Same-
  Connection cycles (pane A → pane B → pane A through one server)
  remain a runtime hazard, mitigated by timeout on `send_and_wait`
  and documented as a mutual-deadlock risk.

- **I9**: Dispatch is cleared (`dispatch.fail_connection()` or `dispatch.clear()`)
  before the handler is dropped. CancelHandle's inverted Drop
  (no-op) depends on this ordering — without it, dropped Dispatch
  entries could reference a destroyed handler during callbacks.

Dispatch entry invariants (request/reply):

- **S1**: Token uniqueness (AtomicU64, per-Connection namespace)
- **S2**: Sequential dispatch — Dispatch callbacks share `&mut H`
  with handler methods, never concurrent (follows from I6/I7)
- **S3**: Control-before-events — RegisterRequest processed before
  any Reply in the same batch
- **S4**: On individual Connection loss, `dispatch.fail_connection()` fires
  for entries keyed to that Connection only, before
  `handler.disconnected()`. On handler destruction, `dispatch.clear()`
  drops all entries across all Connections without firing callbacks.
- **S5**: Cancel removes entry without firing callbacks
- **S6**: `panic = unwind` (follows from I1) — Dispatch HashMap dropped
  during unwind

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

**`send_and_wait`** is the synchronous blocking variant of
`send_request` — it blocks the calling thread until the reply
arrives or a timeout expires. It must NOT be called from a handler
method. Enforcement: `is_looper_thread()` panics on self-deadlock;
`CURRENT_CONNECTION` panics on cross-Connection deadlock.

**I8 is enforced at runtime (panic in all builds).** The looper
maintains a thread-local `CURRENT_CONNECTION` id during handler
dispatch. If `send_and_wait` is called from a handler method, the
runtime panics. The check cost (one thread-local read) is
negligible relative to the IPC round-trip.

**Tokens are per-Connection.** A pane connected to three servers
has three independent token namespaces. `Cancel { token }` is
routed to the Connection that issued the original request. The
Messenger embeds the routing context. Token values may collide
across Connections — this is correct because the namespaces are
disjoint.

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

### Pane exit notification

When a pane exits, the server broadcasts `PaneExited { pane, reason }`
to all other panes on the same Connection. No registration API —
every pane hears about every exit on its Connection. The receiving
pane's filter chain controls what it acts on:

```rust
// A filter that only passes PaneExited for a specific pane.
struct MonitorFilter { target: Id }

impl MessageFilter for MonitorFilter {
    fn matches(&self, msg: &Message) -> bool {
        matches!(msg, Message::PaneExited { .. })
    }
    fn filter(&mut self, msg: &Message) -> FilterAction {
        match msg {
            Message::PaneExited { pane, .. } if *pane == self.target => FilterAction::Pass,
            Message::PaneExited { .. } => FilterAction::Consume,
            _ => FilterAction::Pass,
        }
    }
}
```

The server stays simple (broadcast on Connection), the policy
lives in the client (filter chain). A pane that cares about one
specific peer's exit installs a filter. A pane that doesn't care
about any exits lets the default `pane_exited` handler return
`Ok(Flow::Continue)` — the events arrive and are ignored.

This avoids a registration API (`monitor(target)`) and the
server-side bookkeeping it would require. The filter chain is
the opt-in mechanism.

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

#[pane::protocol_handler(Clipboard)]
impl Editor {
    fn lock_granted(&mut self, _proxy: &Messenger, lock: ClipboardWriteLock) -> Result<Flow> {
        lock.commit(self.buffer.as_bytes().to_vec(), ClipboardMetadata {
            content_type: "text/plain".into(),
            sensitivity: Sensitivity::Normal,
            locality: Locality::Any,
        })?;
        Ok(Flow::Continue)
    }
    fn lock_denied(&mut self, _: &Messenger, _: &str, _: &str) -> Result<Flow> { Ok(Flow::Continue) }
    fn changed(&mut self, _: &Messenger, _: &str, _: Id) -> Result<Flow> { Ok(Flow::Continue) }
    fn service_lost(&mut self, _: &Messenger) -> Result<Flow> { Ok(Flow::Continue) }
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

## Resolved Questions

1. **Service trait dispatch**: fn pointers (Dispatch pattern). Zero-cost,
   handler type known at registration time. Unanimous.

2. **DeclareInterest placement**: Display in handshake (server
   allocates surface resources). Services in active phase (opened
   mid-session via DeclareInterest message).

3. **ClipboardWriteLock::commit()**: Returns `Result<(), CommitError>`
   where `CommitError` includes `Disconnected` and `LockRevoked`.
   Non-negotiable. Unanimous.

4. **Session resumption**: Deferred implementation. The handshake
   type will need a `Resume` path. On resume, the client receives
   `ready()` as if new and must re-send all `DeclareInterest`
   messages. The server MAY optimize by recognizing previously-held
   interests, but the protocol requires re-declaration. Stateful
   obligations (locks, pending requests, open streams) do not
   survive — they fail via Drop compensation. Streams must be
   closed before suspension. The resume path re-negotiates per-
   service protocol versions (the server may have upgraded).
   Obligation-carrying messages must not be buffered across
   reconnection. Tokens expire server-side; client discovers
   expiry at resume time (rejection). Multi-server suspension is
   per-Connection, independent. Reference: Plan 9 aan(8), par
   Server/Proxy/Connection (Michal Strba).

5. **Naming**: `Message` (tier 1 faithful). The type is what BMessage
   was, corrected: obligations extracted to internal types. No
   divergence entry needed.

6. **Service disconnect**: Dual mechanism. `commit() -> Result` for
   synchronous at-call-site detection. `clipboard_service_lost()`
   (and per-service equivalents) on service traits for async
   notification. Scoped to service traits, not base Handler — no
   Handler growth per service.

7. **TLS for remote**: `Connection::remote` requires TLS.
   `PeerAuth::Certificate` derived from TLS certificate. Plaintext
   TCP not supported. No `remote_insecure` escape hatch.

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

13. **Request/reply via Dispatch entries**: `send_request<H, R>` creates
    a one-shot typed dispatch entry in Dispatch. Returns `CancelHandle`
    (Drop = no-op, `.cancel()` = voluntary abort). Replies route to
    callback with typed `R`. Multiple outstanding requests per target
    supported (independent Dispatch entries, unique tokens). No ghost
    state, no `reply_received` on Handler. `dispatch.fail_connection()` before
    `disconnected()`. Six invariants (S1-S6).

14. **RoutingHandler: Handler**: The namespace projection counterpart
    to DisplayHandler (visual projection). Declared via
    DeclareInterest. Handles pane-fs queries and remote commands.
    Phase 3 implementation; trait defined in the main spec.

15. **Geometry**: Logical pixels + scale_factor. `physical_size()`
    helper. Resolves HiDPI ambiguity.

16. **I8 runtime enforcement**: Thread-local CURRENT_CONNECTION in
    looper. `send_and_wait` on different Connection: panic in all
    builds (production deadlock is worse than a crash with backtrace).

17. **Service map precedence**: `$PANE_SERVICE_OVERRIDES` > manifest
    > `$PANE_SERVICES` > `/etc/pane/services.toml`.

18. **DeclareInterest version**: Single `expected_version` for
    Phase 1. Range negotiation (min/max) deferred to Phase 2.

    **Service identity**: Globally unique reverse-DNS names
    (`com.pane.clipboard`) replace hardcoded `SERVICE_ID: u8`.
    Wire discriminants are session-local, assigned by the server
    in `InterestAccepted { session_id: u8 }`. Initial services
    bound in handshake (Hello → Welcome with bindings).
    Late-binding services via active-phase DeclareInterest.

19. **Control protocol shared by Lifecycle and Display**: Intentional.
    Same connection, `ControlMessage` enum (wire service 0).
    Other services get negotiated session-local wire IDs via
    DeclareInterest.

20. **request_received stays untyped**: `Box<dyn Any + Send>` is
    intentional — the server side is fundamentally open (unknown
    senders). Protocol-defined requests route through Handles<P>.
    Ad-hoc inter-pane requests use request_received.

21. **Message enum is base-protocol only**: Service events route
    through `Handles<P>::receive` with per-protocol message types,
    not through the `Message` enum. `Message` covers lifecycle +
    display (the base protocol). New services do not modify it.

22. **PointerPolicy**: Per-pane `Coalesce` (default) or
    `FullHistory` for drawing apps. Server-side filtering.
    Set via `Messenger::set_pointer_policy()`.

23. **Attribute macro exhaustiveness**: `rustc`'s exhaustive match IS
    the guarantee. The `#[pane::protocol_handler(P)]` macro generates
    the match; missing handler methods produce non-exhaustive pattern
    errors at compile time.

24. **Network discovery deferred**: Tier 1 (explicit config) is
    sufficient for all phases. Future discovery mechanisms (mDNS,
    rendezvous) are service map producers — they populate the same
    map, at lowest precedence (explicit config wins). No discovery
    metadata in map entries reaching the App. The service map is
    the abstraction boundary.

25. **No `remote_insecure` escape hatch**: Transport enum is
    `{Unix, Tls}`, no third option. Server refuses plaintext TCP
    on remote listener. `PeerAuth` enum replaces advisory
    PeerIdentity for authorization: `Kernel { uid, pid }` (unix,
    SO_PEERCRED) or `Certificate { subject, issuer }` (TLS).
    `pane_owned_by()` checks `PeerAuth`, not self-reported strings.
    Development certificate tooling (`pane dev-certs`) is a product
    concern — see ops documentation, not this spec.

## Open Questions

None. The spec is implementation-ready for Phase 1.

---

## Phase 1 Structural Invariants

Phase 1 implements Phase 2's data structures populated at N=1.
No shortcuts that create type signatures or invariants Phase 2
must break. If Phase 1's simplifications create invariants that
Phase 2 breaks, Phase 1 was wrong.

| Component | Shortcut to avoid | Correct Phase 1 implementation |
|---|---|---|
| Messenger | Bare `mpsc::Sender` | `ServiceRouter` (HashMap, 1 entry) |
| App | Bare `Connection` field | `HashMap<ServiceName, Connection>` (1 entry) |
| Dispatch | `HashMap<u64, Entry>` | `HashMap<(ConnectionId, u64), Entry>` |
| Wire framing | Flat `[length][payload]` | `[length][service][payload]` (service=0); service IDs negotiated per-connection |
| Message enum | Flat with panic-Clone | Base-protocol only + `#[derive(Clone)]`; service events via `Handles<P>` |
| PeerAuth | Self-reported identity | `PeerAuth::Kernel { uid, pid }` via SO_PEERCRED |
| DeclareInterest | Implicit (connect=display) | Explicit capability declaration |
| ConnectionSource | Pump threads + mpsc | calloop EventSource (read + buffered write) |
| Protocol trait | None | `Lifecycle`, `Display` types exist |
| Handles\<P\> | None | Trait exists, Handler desugars to it, `#[pane::protocol_handler]` attribute macro |
| Dispatch\<H\> + send_request | `reply_received` on Handler | Dispatch HashMap + CancelHandle |
| AppPayload | `T: Send + 'static` | `T: AppPayload` (Clone + Send + 'static) |
| I9 | Not implemented | Dispatch cleared before handler drop |
| max_message_size | Hardcoded | In Hello/Welcome, enforced |
| Cancel { token } | Deferred | Wire message + CancelHandle work |

**Governing principle:** Phase 2 adds entries to Phase 1's data
structures. It does not restructure them. Multi-server is single-
server with N>1 Connections. The compound-key Dispatch, the
ServiceRouter with one entry, the calloop EventSource — these
exist from day one so Phase 2 is additive.

---

## Implementation Phases

**Phase 1 — Core.** Single server (N=1), headless, no suspension,
no streaming. All multi-server data structures present with one
entry (see Phase 1 Structural Invariants). Validate: Protocol +
Handles<P> + derive macro, Handler/DisplayHandler split,
Message/obligation split, Flow, Dispatch<H> for request/reply,
calloop ConnectionSource (read+write), filter chain, Messenger +
ServiceRouter, PeerAuth::Kernel, DeclareInterest, wire framing
[length][service][payload], AppPayload marker, CancelHandle,
max_message_size, I1-I9 + S1-S6. This is the proof that the
linear discipline works end-to-end.

**Phase 2 — Distribution.** Add Connections (N>1). TLS +
PeerAuth::Certificate. Service map full precedence chain.
Version range negotiation. Validate: ServiceRouter with multiple
entries, per-Connection failure isolation, cross-Connection
ordering, multi-server Dispatch (connection_id, token) routing.

**Phase 3 — Lifecycle.** Session suspension/resumption, streaming
(Queue pattern), RoutingHandler. These interact — streams must
close before suspend — so they're designed together.

**Phase 4 — Performance.** Batch coalescing optimizations, write
buffer tuning, connection pooling. Correctness before performance.
