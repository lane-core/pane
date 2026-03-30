# Session-Typed Actor Design Principles for Pane

Derived from analysis of Fowler et al. "Safe Actor Programming with Multiparty Session Types" (EventActors/EAct) against pane's architecture. These principles guide future protocol and API design.

## C1: Heterogeneous Session Loop

The looper must evolve to support multiple typed channels — one per protocol relationship (compositor, clipboard, peer panes, system services). This is the genuine advance over BeOS's BLooper (single kernel port). Each channel has its own message type and dispatches to its own set of handler methods.

**Motivation:** EAct KP3 (multiple sessions). An actor participating in only one session cannot implement server applications or multi-service clients.

**How to apply:** When adding clipboard, DnD, observer, inter-pane messaging, or system services — each is a separate channel into the looper, not a new variant stuffed into CompToClient. Requires multi-source select (crossbeam-channel or calloop multi-source) replacing the current single `mpsc::Receiver<LooperMessage>`.

## C2: Sub-Protocol Typestate, Not Active-Phase Session Types

Don't session-type the `ClientToComp`/`CompToClient` transport. Instead, session-type sub-protocols at the API layer using Rust typestate:

- Clipboard: `ClipboardLock` → write → commit (typestate handle)
- DnD: `DragSession` tracks enter→over→drop progression
- Close negotiation: already implicit in `close_requested()` → `RequestClose` → `CloseAck`
- Completion: token-correlated request/response

**Motivation:** EAct KP2 (no explicit channels) + the paper's own implementation insight — non-blocking state types provide methods, blocking state types are traits. Pane already does this: `Messenger::create_pane()` blocks for PaneCreated internally.

**How to apply:** When designing new sub-protocols, expose typestate handles at the API surface. The transport stays as typed enums. The developer never sees session type machinery.

## C3: Conversation-Level Failure Callbacks

When monitoring another pane, requests in flight should have per-request failure handling, not just global death notification.

**Motivation:** EAct's `suspend` takes an explicit failure callback per conversation. When Shop crashes mid-checkout, Customer's checkout handler gets its specific failure callback — not just "Shop died."

**How to apply:** When implementing inter-pane request-response patterns, the API should accept a failure closure or return a future-like handle that resolves to either the response or a failure. `PaneExited` remains the actor-level signal; conversation-level failure is layered on top.

## C4: Access Points for Service Discovery

EAct's access point ("matchmaking service where actors register for roles, session established when all roles filled") maps to pane's future service registry.

**Motivation:** EAct access points. When clipboard, audio, or notification services exist, panes register interest and sessions are established when both sides are ready.

**How to apply:** Don't build this yet. When designing the service architecture (app registry, system services), use the access point model: actors register for roles, runtime establishes sessions. This is the natural successor to Be's BRoster.

## C5: Handler Declaration of Interest (MessageInterest)

Handlers should declare what message types they expect, enabling runtime verification of coverage and correct routing in a multi-session world.

**Motivation:** EAct handlers are parameterized by their input session type. Pane's Handler trait has defaults for everything — ergonomic but loses static coverage checking.

**How to apply:** Future direction. When the looper supports multiple channels (C1), handlers must declare which channel(s) they service. A `MessageInterest` mechanism or per-channel handler traits.

## C6: Looper = Concurrency Boundary, Session Types = Type Boundary

Keep these orthogonal. The looper's thread model (one thread per pane) is a concurrency concern. Session types are a correctness concern. An actor can participate in multiple sessions on one thread.

**Motivation:** EAct's formalism separates session state (handler store σ) from thread state. The event loop interleaves sessions without requiring multiple threads.

**How to apply:** Don't conflate threading decisions with protocol decisions. Adding clipboard support doesn't mean adding a clipboard thread — it means adding a clipboard channel to the existing looper.
