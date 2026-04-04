# Session-Typed Actor Design Principles for Pane

Derived from analysis of Fowler et al. "Safe Actor Programming with Multiparty Session Types" (EventActors/EAct) against pane's architecture. These principles guide protocol and API design.

## C1: Heterogeneous Session Loop

The looper supports multiple typed channels — one per protocol relationship (server connections, clipboard, peer panes, system services). Each channel has its own message type and dispatches to its own handler method set. calloop multi-source select drives the loop.

**Motivation:** EAct KP3 (multiple sessions). An actor participating in only one session cannot implement server applications or multi-service clients.

**How to apply:** Each service opened via DeclareInterest is a separate typed calloop source. Handler (lifecycle), DisplayHandler (display), and Handles<P> (services) each receive their own protocol's events. Service events never enter the base Message enum.

## C2: Sub-Protocol Typestate, Not Active-Phase Session Types

Session-type sub-protocols at the API layer using Rust typestate. The Protocol trait + Handles<P> dispatch provides the typed surface. The transport stays as per-service wire messages with correlation IDs.

- Clipboard: `ClipboardWriteLock` → commit (typestate handle)
- DnD: `DragSession` tracks enter→over→drop progression
- Request/reply: `ReplyPort` (obligation), `CancelHandle` (option)
- Completion: `CompletionReplyPort` (obligation)

**Motivation:** EAct's type/value separation in handler stores — obligation handles are session endpoints that must not appear in Clone-safe values. Obligation handles provide consuming methods (`.commit()`, `.reply()`). All handler methods are non-blocking (I2).

**How to apply:** When designing new sub-protocols, expose typestate handles at the API surface. The developer never sees session type machinery. `#[pane::protocol_handler(P)]` attribute macro generates dispatch.

## C3: Conversation-Level Failure Callbacks

Per-request failure handling via Dispatch<H>, not just global death notification.

**Motivation:** EAct's `suspend` takes an explicit failure callback per conversation.

**How to apply:** `send_request` registers (on_reply, on_failed) callback pairs as Dispatch entries. On Connection loss, `dispatch.fail_connection()` fires on_failed for entries keyed to the lost Connection only, before `handler.disconnected()`. Entries for other Connections are unaffected. PaneExited (via ExitBroadcaster) remains the actor-level signal; Dispatch provides conversation-level failure.

## C4: Access Points for Service Discovery

EAct's access point model maps to pane's DeclareInterest + ServiceRouter. Services are identified by ServiceId (UUID + reverse-DNS name). The server assigns session-local wire discriminants during DeclareInterest negotiation.

**Motivation:** EAct access points. When services exist, panes declare interest and sessions are established when both sides are ready.

**How to apply:** DeclareInterest in the active phase (or interests list in Hello for initial services). Service map from environment provides discovery. Access point model natural for future BRoster equivalent.

## C5: Handler Declaration of Interest (Handles<P>)

Handlers declare what protocol types they handle via `impl Handles<P>`. The `PaneBuilder::open_service::<Clipboard>()` call requires `H: Handles<Clipboard>` at compile time — the bound IS the interest declaration.

**Motivation:** EAct handlers are parameterized by their input session type.

**How to apply:** Protocol::Message + Handles<P> + attribute macro provides compile-time coverage checking. If a pane declares interest in a service (via PaneBuilder), its handler must implement the corresponding Handles<P>. Missing method → compile error (exhaustive match in generated dispatch).

## C6: Looper = Concurrency Boundary, Session Types = Type Boundary

Keep these orthogonal. The looper's thread model (one thread per pane) is a concurrency concern. Session types are a correctness concern. An actor can participate in multiple sessions on one thread.

**Motivation:** EAct's formalism separates session state (handler store σ) from thread state.

**How to apply:** Don't conflate threading decisions with protocol decisions. Adding clipboard support means adding a clipboard calloop source to the existing looper and implementing Handles<Clipboard> — not adding a clipboard thread.
