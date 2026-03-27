# Research: Multiparty Session Types for Pane

Analysis of "Speak Now: Safe Actor Programming with Multiparty Session Types" (Fowler & Hu, OOPSLA) and its implications for pane-session.

---

## Part 1: Paper Analysis

### What Maty Is

Maty is a core actor language with static multiparty session typing. The name stands for "multiparty actors, typed." The paper introduces the first actor language design that supports both static MPST checking and actors participating in multiple sessions simultaneously.

The central tension the paper resolves: session types were designed for channel-based languages (Go, Concurrent ML), where anonymous processes communicate over typed channel endpoints. Actor languages (Erlang, Akka) use a fundamentally different model -- named processes, single mailbox per process, reactive message handling. These two worlds have different strengths:

- **Channels**: easy to type, hard to distribute (sending channel endpoints requires distributed delegation)
- **Actors**: easy to distribute, hard to type (mailboxes receive from many senders, typing requires large variant types)

Maty bridges the gap by making actors session-typed without exposing channels to the programmer.

### Event Actors: The Core Idea

An "event actor" is an actor that participates in sessions through an event-driven programming model rather than blocking receive operations:

1. An actor **registers** with an **access point** to join a session, providing a role name and an initialization callback.
2. When all roles are filled, the access point **establishes** the session and invokes each actor's callback.
3. Within a session, an actor can **send** messages directly (non-blocking). When it needs to **receive**, it does not block -- instead, it **suspends** by installing a **message handler** and yielding to the event loop.
4. The event loop can then invoke any installed handler for any session that has a pending message. This is how one actor participates in multiple sessions: each session has its own handler, and the event loop multiplexes across them.

The key insight: **there is no receive primitive**. Reception is implicit -- it happens when the event loop dispatches to a handler. This eliminates the deadlock problem inherent in multi-session actors: a blocking receive on session A prevents the actor from servicing session B, creating potential inter-session deadlocks. With event-driven handlers, the actor is always ready to service any session.

This maps exactly to Erlang's gen_server pattern and Akka's Behaviors.receive -- computation is triggered by message arrival, not by explicit receive calls.

### Multiparty Session Types vs. Binary

Pane currently has binary session types: `Send<A, S>`, `Recv<A, S>`, `End`, with duality (`Dual<S>`). Two parties, one channel.

Multiparty session types (MPSTs) extend this to N parties. The key differences:

**Global types** describe the entire protocol from a bird's-eye view:
```
Client -> Server : { IDRequest() . Server -> Client : { IDResponse(Int) . G, Unavailable() . G },
                     LockRequest() . Server -> Client : { Locked() . AwaitUnlock, Unavailable() . G },
                     Quit() . end }
AwaitUnlock = Client -> Server : { Unlock() . G }
```

**Local types** are projections of the global type onto each role. The server's local type describes what the server sees:
```
ServerTy = Client & { IDRequest() . Client (+) { IDResponse(Int) . ServerTy, Unavailable() . ServerTy },
                      LockRequest() . Client (+) { Locked() . ServerLockTy, Unavailable() . ServerTy },
                      Quit() . end }
```

Where `&` means "offer a choice (receive)" and `(+)` means "make a selection (send)." Each local type is annotated with which **role** the interaction targets.

**Duality becomes coherence.** In binary session types, the two endpoints are duals. In MPST, the relationship is more complex: each role's local type must be *compatible* with all others. This compatibility is checked via the **compliance** property (see below), not simple duality.

**Protocols as role maps.** A protocol P = {Client: ClientTy, Server: ServerTy} maps role names to local types. Access points are parameterized by protocols. When an actor registers for role Server on an access point for protocol P, the system knows the actor must follow ServerTy.

### How Maty Handles Nondeterminism

This is the critical contribution and the reason this paper matters for pane.

**The problem:** In standard MPST process calculi, a server that handles N clients spawns a separate process per client session, each following the session type independently. But if sessions need to interact (shared state, resource coordination), the standard approach either (a) introduces additional internal sessions between the processes (losing deadlock-freedom guarantees), or (b) uses out-of-band synchronization (losing session-typing guarantees).

**Maty's solution:** A single actor participates in multiple sessions through the event loop. When multiple sessions have pending messages, the event loop can invoke any handler -- this is controlled nondeterminism. The actor's internal state mediates between sessions.

Concretely: the ID server has one actor handling N client sessions. Each client is in its own session (with its own session type state), but the actor processes them one at a time through the event loop. The `locked` flag is actor-level state, not session-level state. When a client acquires the lock, the actor's state changes, and subsequent clients see `Unavailable` -- but this is decided by the actor's internal logic, not by the session type.

**The branching type is the key mechanism.** The session type `Client (+) { IDResponse(Int) . G, Unavailable() . G }` says the server CHOOSES which branch. The type does not dictate which branch -- the actor's internal state does. But the type guarantees that whatever the server chooses, both parties agree on the continuation.

**What Maty does NOT solve:** The types do not constrain which branch the actor takes based on internal state. The connection between `locked == true => Unavailable` is in the actor's code, not the type system. The types only guarantee: (1) the server will send one of the specified messages, (2) the client will handle all possible responses, (3) the session advances consistently after the choice.

### How Maty Handles Crashes / Failure

Section 4 of the paper extends Maty to Maty_lightning (with failure handling). The approach is **affine sessions with cascading failure**:

1. **raise** terminates the actor and cancels all its session endpoints.
2. A cancelled endpoint produces a **zapper thread** (`lightning(s[p])`) that propagates through the system.
3. **E-CancelMsg**: queued messages for a cancelled role are discarded.
4. **E-CancelH**: when an actor is waiting for a message from a cancelled role (and the queue for that sender is empty), the actor's **failure continuation** is invoked.
5. **suspend** is extended to take a third argument: a failure callback `V_exn` to be invoked if the sender crashes.
6. **monitor(a, f)** installs a callback `f` to be invoked if actor `a` crashes. This is Erlang's process monitoring.

The failure model is:
- Failure cascades through sessions (if one participant crashes, others are notified)
- Supervisors can restart crashed actors (the `shopSup` pattern)
- The metatheory (preservation, progress) holds even with failure

**Critical for pane:** The paper's suspend-with-failure-continuation maps directly to pane-session's `Err(SessionError::Disconnected)` pattern. The difference: Maty makes the failure continuation explicit per-handler, while pane makes it a `Result` at every operation. Pane's approach is more uniform but less expressive -- you can't install different failure behaviors for different session states.

### The Type System

Maty uses a **flow-sensitive effect system** to enforce session typing. The typing judgment for computations is:

```
Gamma | C |- S > M : A < T
```

Read: "Under type environment Gamma, with actor state type C, given session precondition S, term M has type A and postcondition T."

Key rules:

- **T-Send**: `p ! l(V)` is typable when the precondition is `p (+) { l_i(A_i) . S_i }` and l = l_j. Postcondition becomes S_j.
- **T-Suspend**: `suspend V_h V_s` is typable when V_h is a handler for the current input session type, V_s is the new actor state. Since suspend does not return (it yields to the event loop), it can be given an arbitrary return type and postcondition.
- **T-Handler**: A handler `handler p st { l_i(x_i) => M_i }` has type `Handler(p & { l_i(A_i) . S_i }, C)` if each branch M_i is typable with precondition S_i and postcondition `end`.
- **T-Register**: Registering for role p on an access point checks that the callback has the right session type for role p.
- **T-NewAP**: Creating an access point requires that the protocol is **compliant**.

The postcondition `end` on handler branches is critical: it means each handler branch must either finish the session or suspend (yield to the event loop with a new handler installed). This prevents a handler from "hoarding" the session -- it must either complete its obligations or relinquish control.

There is no receive construct in the type system. Reception is implicit through the event loop.

### Key Theorems

**Theorem 1 (Preservation).** Typability is preserved by structural congruence and reduction. If a configuration is well-typed and reduces, the result is well-typed. Consequence: an actor will never send or receive a message of the wrong type.

**Theorem 2 (Progress).** A well-typed configuration either reduces or is in a terminal state (all actors idle, no active sessions). Consequence: no session gets stuck because of a protocol design error.

**Corollary 1 (Global Progress).** If all event handlers terminate, then for every ongoing session, communication is eventually possible. Consequence: with terminating handlers, the system makes progress on every session -- no starvation.

**Theorem 3 (Preservation, Maty_lightning).** Preservation holds even with failure and cascading cancellation.

**Theorem 4 (Progress, Maty_lightning).** Progress holds with failure: a configuration either reduces, or all sessions have either completed or been cancelled.

**Theorem 5 (Global Progress, Maty_lightning).** Every ongoing session either advances or gets cancelled.

The key condition for all of these: **compliance** of the protocol. A protocol is compliant if it is safe (no communication mismatches) and deadlock-free (every message eventually gets received).

Checking compliance for asynchronous protocols is undecidable in general (Scalas & Yoshida 2019), but decidable for protocols obtained by projection from global types, or checkable by bounded model checking (Scribble).

### The Chat Server Problem

The paper addresses this in Section 5.2 (evaluation). The chat server decomposes into three protocols:

1. **ChatServer(C, S)**: Client-Registry interaction -- lookup room, create room, list rooms, bye.
2. **ChatSessionCtoR(C, R)**: Client sends messages to Room -- outgoing messages or leave.
3. **ChatSessionRtoC(R, C)**: Room broadcasts to Client -- incoming messages or bye.

The key insight: **the bidirectional client-room interaction is decomposed into two separate unidirectional protocols**. The Room actor participates in N instances of ChatSessionRtoC (one per connected client) and handles ChatSessionCtoR messages through the event loop. Broadcasting requires `ibecome` -- the Room freezes its current session context, switches into each ChatSessionRtoC session to send the message, and resumes.

This is the "server pushes to clients" pattern. The Room has N ongoing ChatSessionRtoC sessions. When it receives an OutgoingChatMessage from any client, it must forward to all other clients. The event-driven model makes this possible: the Room's handler for ChatSessionCtoR can, during its execution, send messages on other sessions (ChatSessionRtoC) before suspending.

### Global Types to Local Types

Global types provide a top-down specification. Projection produces local types -- one per role. Projection is standard (Honda, Yoshida, Carbone 2008): for each communication `A -> B : { l_i(T_i) . G_i }` in the global type:

- Role A gets: `B (+) { l_i(T_i) . proj(G_i, A) }` (selection -- A sends)
- Role B gets: `A & { l_i(T_i) . proj(G_i, B) }` (offer -- B receives)
- Other roles get: `proj(G_i, C)` (must be the same for all branches i)

The paper's formalism is based on local types directly (following Scalas & Yoshida 2019) rather than requiring global types. The compliance check on local types is sufficient -- global types are convenient for specification but not necessary for the metatheory. This is important: it means you can write local types directly and check compliance, without needing to first write a global type and project.

---

## Part 2: Implications for Pane

### The Compositor IS a Multiparty Session

Pane's compositor talks to multiple clients simultaneously. Each client has its own pane thread. But the compositor's main thread (calloop) is the shared state coordinator -- it is an actor in exactly Maty's sense.

Consider the real interaction pattern:

1. Client A sends a content update for its pane.
2. The compositor's pane thread for A processes the message, needs to update the layout tree (shared state on the main thread).
3. Meanwhile, Client B sends a resize request.
4. The compositor sends focus events to Client A (push from server to client).

This IS the multiparty session problem. The compositor main thread is mediating between N client sessions, and its internal state (layout tree, focus tracking) determines what it sends to each client. Binary session types model each compositor-client pair independently but cannot express the relationship between them.

**Verdict: Pane needs multiparty thinking, but does not need multiparty session types in the type system.**

Here is why the distinction matters:

Maty's formal multiparty types require:
- A fixed, known set of roles per protocol (Client, Server, PaymentProcessor)
- A global type specifying all interactions between all roles
- Projection producing compatible local types

Pane's compositor has:
- A dynamically varying number of clients (N is not statically known)
- No meaningful global type that spans all client interactions
- Binary relationships (compositor <-> client) that are mediated by shared state

This is exactly the situation the ID server paper describes: N clients, one server, shared state. Maty handles it not through an N-party global type, but through the server registering for N binary sessions on the same access point and mediating through internal state.

**What pane should adopt from Maty: the event-driven actor model for the compositor main thread. What it should not adopt: formal multiparty session types as type constructors.**

### The Nondeterminism Problem Maps Directly

The compositor sends events (resize, focus, input) to clients while clients send content updates to the compositor. This bidirectional communication is the nondeterminism problem.

In pane's current binary session types, a protocol like:
```rust
type PaneProtocol = Send<Content, Recv<Resize, Send<Content, Recv<Focus, End>>>>
```
forces a fixed alternation. But the real protocol is: either side can send at any time during the "active" phase.

**Maty's approach to this (from the chat server): decompose into separate unidirectional sessions.**

For pane:
- **ClientToCompositor**: `mu X. Choose { ContentUpdate(data) . X, RequestClose() . End, ... }`
- **CompositorToClient**: `mu X. Choose { Resize(rect) . X, Focus(bool) . X, InputEvent(ev) . X, ... }`

Two separate sessions on the same unix socket (multiplexed by message tags). Each is independently session-typed with clear directionality.

**This is actually what pane's architecture already implies with "async by default."** The compositor sends events; the client sends content. They don't wait for each other. The session types for each direction are independent.

**Verdict: Adopt protocol decomposition into unidirectional sessions. Do not try to type the full bidirectional active phase as a single binary session type.**

### Per-Pane Threads ARE the Actor Topology

The architecture spec says: "Each pane gets its own server-side thread." These per-pane threads coordinate through the compositor main thread. This is precisely Maty's actor topology:

- Each pane thread is an actor
- The compositor main thread is an actor
- Per-pane threads send messages to the main thread via channels
- The main thread sends responses/events back

The main thread is the event loop (calloop). When it receives a message from any pane thread, it processes it and may send events to other pane threads. This is exactly E-React from Maty: the idle actor has handlers for each session, and the event loop invokes whichever handler has a pending message.

**What pane should adopt:** Model the compositor main thread explicitly as a Maty-style actor with installed handlers per session. The calloop event sources are the access points. Each connected client is a session. The main thread's handler state maps each session to its current protocol position.

**What this means for pane-session:** The calloop SessionSource is already essentially a handler registration mechanism. The missing piece is the notion of **per-session protocol state** tracked on the server side. Currently, the compositor side is untyped (it receives raw bytes via SessionEvent::Message). The paper's approach suggests: each session should have associated state that tracks its protocol position, and the handler should be parameterized by this state.

### Dynamic Specifier Chains

The scripting protocol needs dynamic dispatch across handler boundaries. Does Maty's formalism help?

Partially. The specifier chain "get Frame of Window 1 of Application Tracker" is a sequence of handler invocations, each peeling off one specifier. In Maty terms, this is a recursive session type:

```
ScriptingProtocol = mu X. Client (+) {
    Resolve(specifier) . Server & {
        Resolved(value) . X,
        Forward(remaining) . X,
        NotFound(error) . End
    }
}
```

Each step is binary (client <-> current handler). Forwarding changes the handler but preserves the session type. This maps to Maty's handler switching -- the `become` pattern where one handler installs a different handler for the next message.

**Verdict: The scripting protocol can use binary session types with handler switching. Multiparty types are not needed here. The dynamic composition happens at the handler level (optic resolution), not at the session type level.**

### Crash Safety

Maty's failure model (`raise`, cascading cancellation, supervisor monitoring, failure continuations on `suspend`) maps closely to pane's existing crash handling:

| Maty | Pane |
|------|------|
| `raise` | Thread panic + channel drop |
| `lightning(s[p])` (zapper) | `SessionError::Disconnected` |
| E-CancelMsg (discard queued messages) | Unix socket buffer discarded on close |
| E-CancelH (invoke failure continuation) | `recv() -> Err(Disconnected)` |
| `monitor(a, f)` | pane-watchdog heartbeat monitoring |
| `suspend V_h V_s V_exn` | No direct equivalent (see below) |

**The gap:** Maty's suspend takes a per-handler failure continuation. Pane treats all failures uniformly as `Result::Err`. The Maty approach is more expressive -- you can install different recovery logic depending on which protocol state the failure occurs in.

For pane's compositor: when a client dies while the compositor is waiting for a content update (state A) vs. when it dies during a handshake (state B), the cleanup might differ. Currently pane handles both cases identically (remove panes, continue). If this remains sufficient, the uniform `Result::Err` approach is fine. If not, Maty's per-state failure continuations are the right upgrade path.

**Verdict: Pane's current crash handling (`Err` not panic) is sufficient. If per-state failure recovery becomes needed, add failure callbacks to the calloop handler registration, not to the session types themselves.**

### Compositional Equivalence

The compositional equivalence invariant requires: for any composition relationship (a split in the layout tree), a script must be able to discover, query, and dissolve it through the standard protocol without special-case APIs.

Maty's global types help here as a specification tool. A global type for the composition protocol would specify exactly what messages flow between the compositor and clients when composition relationships change:

```
Global protocol CompositionChange(role Compositor, role ClientA, role ClientB):
    Compositor -> ClientA: Resize(new_rect)
    Compositor -> ClientB: Resize(new_rect)
    // Clients acknowledge
    ClientA -> Compositor: Resized()
    ClientB -> Compositor: Resized()
```

This is a genuine 3-party interaction: the compositor orchestrates a change that affects two clients. But the projection onto each client is binary (ClientA <-> Compositor, ClientB <-> Compositor), and the compositor mediates the coordination through its internal state. This is exactly the Maty pattern: N binary sessions, one mediating actor.

**Verdict: Use global types as specification language for multi-client coordination scenarios (composition changes, focus handoff, drag between panes). Implement as N binary sessions mediated by the compositor actor. Do not implement multiparty type constructors.**

---

## Part 3: Concrete Recommendations for pane-session

### Should Pane Move from Binary to Multiparty Session Types?

**No. Keep binary session types. Use multiparty thinking at the design level.**

Reasons:

1. **The N is dynamic.** The number of clients is not statically known. Maty handles this through repeated registration on access points, not through N-ary type constructors. Binary types with the event-driven actor pattern on the compositor side achieve the same thing.

2. **Implementation complexity.** Multiparty session types in Rust would require role-indexed type constructors, per-role transport management, and a compliance checker. The payoff does not justify the cost for pane's use case.

3. **The compositor already has the right structure.** calloop + per-client SessionSource + shared state via channels IS the Maty actor model, implemented without the type formalism. Adding binary session types to each client connection gives per-connection protocol safety. The cross-connection coordination is the compositor's internal logic, which Rust's ownership system already constrains.

4. **What multiparty buys you -- deadlock freedom across sessions -- pane gets for free from the event-driven model.** The compositor never blocks on a single client; calloop multiplexes. This is exactly Maty's insight: event-driven actors cannot inter-session deadlock because they never block.

### What New Type Constructors Are Needed

Two additions to pane-session's type vocabulary:

**1. Choose / Offer (branching)**

Currently missing. These are the `(+)` and `&` from the paper's local types.

```rust
/// Session type: the sender CHOOSES which branch.
/// In linear logic: A (+) B (plus -- internal choice).
///
/// The sender selects a variant from enum E and sends it.
/// The continuation depends on which variant was selected.
pub struct Choose<E>(PhantomData<E>);

/// Session type: the receiver OFFERS all branches.
/// In linear logic: A & B (with -- external choice).
///
/// The receiver gets an enum E and must handle all variants.
/// The continuation depends on which variant was received.
pub struct Offer<E>(PhantomData<E>);
```

The enum E carries the session continuations:

```rust
/// Example: compositor offers client two possibilities after handshake
#[derive(Serialize, Deserialize)]
enum ServerDecision<S1, S2> {
    Accepted(AcceptData),  // continues as S1
    Rejected(RejectReason), // continues as S2
}
```

However, implementing this cleanly in Rust requires thought. The enum variants carry different continuation types, so the enum itself is parameterized by session types. The cleanest approach is to have Choose send a tag byte followed by the variant payload, and have the continuation type be selected by the tag.

In practice, the simplest working approach for pane:

```rust
/// A choice point: sender picks one of N labeled branches.
/// Implemented as: send a tag (u8/u16), then continue as
/// the corresponding session type.
///
/// For two branches:
pub struct Select<A, B>(PhantomData<(A, B)>);
pub struct Branch<A, B>(PhantomData<(A, B)>);

impl<A, B, T: Transport> Chan<Select<A, B>, T> {
    /// Select the left branch, returning the continuation.
    pub fn left(self) -> Result<Chan<A, T>, SessionError> { ... }
    /// Select the right branch, returning the continuation.
    pub fn right(self) -> Result<Chan<B, T>, SessionError> { ... }
}

impl<A, B, T: Transport> Chan<Branch<A, B>, T> {
    /// Receive the peer's choice. Returns either Left(Chan<A, T>)
    /// or Right(Chan<B, T>). Exhaustive -- must handle both.
    pub fn branch(self) -> Result<BranchResult<A, B, T>, SessionError> { ... }
}

pub enum BranchResult<A, B, T: Transport> {
    Left(Chan<A, T>),
    Right(Chan<B, T>),
}
```

For N-ary choice, nest: `Select<A, Select<B, Select<C, End>>>`. Or use a macro to generate flat N-ary versions.

Duality: `Dual<Select<A, B>> = Branch<Dual<A>, Dual<B>>`.

**2. Rec / Var (recursion)**

Currently missing. Needed for the "active phase" where messages repeat.

```rust
/// Recursive session type: mu X. S
/// Binds type variable X in continuation S.
pub struct Rec<F>(PhantomData<F>);

/// Type variable reference (unfolds to the enclosing Rec).
pub struct Var;
```

Rust's type system makes recursive types difficult. The pragmatic approach: use `type` aliases with explicit recursion:

```rust
// Active phase: client can send content updates or close, repeatedly
type ActiveClient = Select<
    Send<ContentUpdate, Rec<ActiveClient>>,  // send update, loop
    Send<CloseRequest, End>,                  // close
>;
```

But `Rec<ActiveClient>` is a cycle. In practice, use an enum for the protocol state machine rather than recursive types:

```rust
// The pragmatic Rust approach: protocol as enum + state machine
#[derive(Serialize, Deserialize)]
enum ClientMessage {
    ContentUpdate(ContentData),
    CloseRequest,
}

// Session type for the repeating phase:
// repeatedly: client sends ClientMessage, compositor responds, loop
type ActivePhase = Send<ClientMessage, Recv<CompositorResponse, ActivePhase>>;
```

This hits Rust's recursion limit. The practical fix: use a finite unrolling or, better, **exit the session type system for the steady-state phase** and use the calloop handler pattern with typed messages.

**Recommendation: For the steady-state active phase, use typed message enums dispatched through calloop handlers, NOT recursive session types.** Reserve session types for the phases with non-trivial structure: handshake, negotiation, teardown. The active phase where either side can send at any time is better modeled as Maty-style event-driven message handling with typed enums.

### How Should Choose/Offer Be Implemented Given the Paper's Approach?

Maty uses `(+)` (select) and `&` (offer) with labeled branches. The label is a message tag. The sender selects a label; the receiver pattern-matches on all possible labels.

For pane-session, the wire format:

```
[1 byte: branch tag] [N bytes: variant payload]
```

The implementation:

```rust
impl<L, R, T: Transport> Chan<Select<L, R>, T> {
    /// Select the left branch. Sends tag 0, advances to L.
    pub fn select_left(mut self) -> Result<Chan<L, T>, SessionError> {
        self.transport.send_raw(&[0u8])?;
        Ok(self.advance())
    }

    /// Select the right branch. Sends tag 1, advances to R.
    pub fn select_right(mut self) -> Result<Chan<R, T>, SessionError> {
        self.transport.send_raw(&[1u8])?;
        Ok(self.advance())
    }
}

impl<L, R, T: Transport> Chan<Branch<L, R>, T> {
    /// Receive the peer's choice.
    pub fn offer(mut self) -> Result<Either<Chan<L, T>, Chan<R, T>>, SessionError> {
        let tag = self.transport.recv_raw()?;
        match tag.as_slice() {
            [0] => Ok(Either::Left(self.advance())),
            [1] => Ok(Either::Right(self.advance())),
            _ => Err(SessionError::Codec(postcard::Error::DeserializeBadEncoding)),
        }
    }
}
```

Duality for branching:

```rust
impl<L: HasDual, R: HasDual> HasDual for Select<L, R> {
    type Dual = Branch<Dual<L>, Dual<R>>;
}

impl<L: HasDual, R: HasDual> HasDual for Branch<L, R> {
    type Dual = Select<Dual<L>, Dual<R>>;
}
```

### How Should the Active Phase Be Typed?

The nondeterministic active phase (compositor <-> client bidirectional messaging) is the hardest problem. The paper's answer: decompose into separate unidirectional sessions.

For pane, the concrete design:

**Protocol phases:**
```
Handshake -> Negotiation -> Active -> Teardown
```

**Handshake** (binary session type, strict ordering):
```rust
type PaneHandshake = Send<ClientHello, Recv<ServerHello, Send<PaneConfig, Recv<PaneAck, ActivePhaseTransition>>>>;
```

**ActivePhaseTransition** transitions from session-typed to event-driven:
```rust
// After handshake, the channel splits conceptually:
// - Client-to-compositor: typed enum messages, async
// - Compositor-to-client: typed enum events, async
// The unix socket carries both directions, demuxed by a direction tag.

#[derive(Serialize, Deserialize)]
enum ClientToComp {
    ContentUpdate(ContentData),
    TagUpdate(TagContent),
    RequestClose,
    ScriptingQuery(SpecifierChain),
}

#[derive(Serialize, Deserialize)]
enum CompToClient {
    Resize(Rect),
    Focus(bool),
    InputEvent(InputData),
    TagRoute(RouteInfo),
    ScriptingResponse(ScriptResult),
}
```

**Why this works:** During the active phase, both sides send typed enums. The session-type level guarantee (ordering) is replaced by the message-level guarantee (every enum variant is handled). This is exactly Maty's handler approach: the handler takes a typed message enum and pattern-matches exhaustively.

**The calloop integration on the compositor side already does this.** SessionSource delivers `SessionEvent::Message(bytes)`, which the handler deserializes and matches. Making this typed:

```rust
// Instead of raw bytes, the calloop handler receives typed messages
handle.insert_source(source, move |event, _, state| {
    match event {
        SessionEvent::Message(bytes) => {
            let msg: ClientToComp = postcard::from_bytes(&bytes)?;
            match msg {
                ClientToComp::ContentUpdate(data) => { /* ... */ }
                ClientToComp::TagUpdate(content) => { /* ... */ }
                ClientToComp::RequestClose => { /* ... */ }
                ClientToComp::ScriptingQuery(chain) => { /* ... */ }
            }
            Ok(PostAction::Continue)
        }
        SessionEvent::Disconnected => { /* cleanup */ Ok(PostAction::Remove) }
    }
});
```

**Verdict: Use binary session types for structured phases (handshake, negotiation, teardown). Use typed message enums with calloop handlers for the active phase. This is Maty's approach translated to Rust without the formal effect system.**

### Does the Paper's Global Type Approach Help Compositional Equivalence?

Yes, as a specification tool. Not as an implementation mechanism.

The compositional equivalence invariant says: any composition operation visible in the layout tree must be discoverable and manipulable through the protocol and filesystem. A global type for composition operations would serve as the specification:

```
Global protocol SplitCreate(role Compositor, role PaneA, role PaneB):
    Compositor -> PaneA: Resize(new_rect_a)
    Compositor -> PaneB: Resize(new_rect_b)
    PaneA -> Compositor: Ack
    PaneB -> Compositor: Ack
```

This global type makes explicit: a split affects two panes, and both must acknowledge. Projection gives each pane a binary local type (it just sees a resize), and the compositor's local type shows it must message both panes.

Use global types as documentation and specification. The protocol documentation should include global types for multi-client scenarios, expressed in a human-readable notation. This catches design errors early: if you can't write a global type for a feature, the protocol design might have a coherence problem.

**Do not generate code from global types.** The Scribble toolchain (which Maty's implementation uses) generates state machine APIs from global types. This is appropriate for distributed systems where all parties are independent. Pane's compositor is centralized; code generation adds complexity without proportional benefit.

### Concrete Implementation Plan

**Phase 1: Add Choose/Offer to pane-session (immediate)**

Add `Select<L, R>`, `Branch<L, R>` with duality. Wire format: 1-byte tag prefix. Test with the existing memory and unix transports.

```rust
// New file: crates/pane-session/src/types.rs additions

/// Choice: this endpoint selects one of two continuations.
pub struct Select<L, R>(PhantomData<(L, R)>);

/// Offer: this endpoint receives and handles the peer's choice.
pub struct Branch<L, R>(PhantomData<(L, R)>);

/// Result of receiving a branch choice.
pub enum Either<L, R> {
    Left(L),
    Right(R),
}
```

**Phase 2: Define pane protocol phases (next)**

In pane-proto, define the handshake and negotiation phases as session types. Define the active phase as typed message enums.

```rust
// crates/pane-proto/src/session.rs

/// Phase 1: Client sends hello, server responds
type Handshake = Send<ClientHello, Recv<ServerHello, Negotiation>>;

/// Phase 2: Negotiate capabilities
type Negotiation = Send<Capabilities,
    Branch<
        Recv<Accepted, ActiveTransition>,  // server accepts
        Recv<Rejected, End>,               // server rejects
    >>;

/// Phase 3 transition: session types end, typed enums take over
type ActiveTransition = Send<Ready, End>;

// After ActiveTransition, both sides communicate via typed enums
// on the same socket, demuxed by message type tags.
```

**Phase 3: Event-driven handler model for compositor (later)**

Formalize the compositor's main loop as a Maty-style actor. Each connected client has per-session state tracking:

```rust
struct PaneSession {
    /// Which phase of the protocol this session is in
    phase: PanePhase,
    /// The session's server-side pane state
    pane: PaneState,
    /// Handler for the current phase (determines what messages are valid)
    handler: Box<dyn PaneHandler>,
}

enum PanePhase {
    Handshake,
    Negotiation,
    Active,
    Teardown,
}
```

This is not session-typed in the Rust type system (the phase transitions happen at runtime). But the session types from Phase 2 guarantee that the handshake and negotiation phases are well-formed, and the typed enums from Phase 2 guarantee that active-phase messages are well-formed. The compositor's handler state machine is the runtime analog of Maty's handler state.

**Phase 4: Per-state failure continuations (if needed)**

If uniform `Err(Disconnected)` proves insufficient, add optional failure callbacks to the calloop handler registration:

```rust
struct SessionRegistration {
    source: SessionSource,
    on_message: Box<dyn FnMut(SessionEvent) -> io::Result<PostAction>>,
    on_failure: Option<Box<dyn FnMut(PanePhase) -> io::Result<()>>>,
}
```

The failure callback receives the phase the session was in when the client died, enabling phase-specific cleanup.

### Summary of Adopt / Adapt / Ignore

| Paper Concept | Recommendation | Rationale |
|---|---|---|
| Event-driven actor model | **Adopt** | The compositor main thread IS this already via calloop. Formalize it. |
| Suspend / handler pattern | **Adopt** | calloop's callback registration IS suspend. Make the parallel explicit. |
| Access points | **Ignore** | Pane uses unix socket accept(), which serves the same purpose. |
| Global types | **Adapt** as specification language | Use for protocol documentation, not code generation. |
| Local types / projection | **Ignore** at type level | Binary session types suffice. Use projection mentally during protocol design. |
| Choose/Offer (branching) | **Adopt** | Add Select/Branch to pane-session. Needed for protocol negotiation. |
| Compliance checking | **Ignore** at runtime | Pane's protocols are small and hand-verified. Scribble-style tooling is overkill. |
| Flow-sensitive effect system | **Ignore** | Rust's typestate pattern (Chan<S, T>) achieves the same per-channel. |
| Affine sessions / raise | **Already implemented** | pane-session's Err(Disconnected) IS the affine session approach. |
| Cascading cancellation | **Adapt** | The compositor already handles this (remove dead client's panes). Make it systematic. |
| Monitor / supervisor | **Adapt** | pane-watchdog IS the supervisor. Formalize the restart protocol. |
| Failure continuations on suspend | **Defer** | Add only if uniform Err handling proves insufficient. |
| Protocol decomposition (chat pattern) | **Adopt** | Decompose active phase into unidirectional typed enum streams. |
| ibecome (session switching) | **Ignore** | Not needed -- the compositor's calloop multiplexes sessions naturally. |
| Recursive session types | **Ignore** in types | Use typed enums for repeating phases. Recursive types in Rust are impractical. |

### Key Takeaway

Maty validates pane's existing architecture. The compositor's calloop + per-pane threads + typed messages is the Maty actor model realized in Rust without the formal type machinery. The paper's strongest contribution for pane is not the type system itself but the **decomposition strategy**: structured phases (handshake, negotiation, teardown) get session types; the steady-state active phase gets typed message enums with event-driven handlers. The multiparty coordination happens in the compositor's internal logic, mediated by its shared state, exactly as Maty prescribes.

The immediate actionable items:
1. Add `Select`/`Branch` to pane-session
2. Define the pane protocol as phased: session-typed structured phases + enum-typed active phase
3. Document cross-client coordination scenarios as global types in the protocol specification
4. Formalize the compositor main thread as a Maty-style event actor (in documentation, not in types)
