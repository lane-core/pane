# Assessment: Build vs. Buy for Session Types

An engineering assessment of whether pane should use par, dialectic, or a custom session type implementation. Written from the perspective of someone who shipped a real-time desktop OS with pervasive message-passing and is now looking at the tools available to do it again with static guarantees.

---

## 1. par: What pane would use vs. fight against

### What par gets right

Par's core design is genuinely elegant. The Session trait with its Dual involution, enum-based branching (no special Choose/Offer types, just Rust enums), recursion through recursive enums with `Recv`/`Send` providing heap indirection via internal oneshot channels, and the `()` as self-dual terminal session -- these are clean and idiomatic. The Wadler CP correspondence is correctly implemented. The crate is small, auditable, and does what it says on the tin.

The Queue module (`Dequeue`/`Enqueue`) is a well-designed primitive for streaming that maps directly to pane's model of batched drawing commands. The server module's Server/Proxy/Connection pattern addresses the multi-client problem that a compositor inherently has.

### What pane would fight against

**Transport: par is in-memory only.** This is the fundamental mismatch. Par's `Send` and `Recv` are wrappers around `futures::channel::oneshot`. The `send()` method creates a fresh oneshot pair for the continuation, sends the value and one end through the current oneshot, and returns the other end:

```rust
pub fn send(self, value: T) -> S {
    S::fork_sync(|dual| {
        self.tx.send(Exchange::Send((value, dual)))
            .ok()
            .expect("receiver dropped")
    })
}
```

Every send allocates a new oneshot channel. The continuation endpoint is physically passed through the channel alongside the value. This is the mechanism that makes the session type "advance" -- after sending, you get a fresh endpoint typed at the next protocol state.

For in-memory use, this is correct and efficient. For pane, it means par cannot be the wire transport. You cannot serialize a `futures::channel::oneshot::Sender` across a unix socket. The continuation channel is an in-memory object, not a serializable token.

Pane needs to send postcard-serialized messages over unix sockets. The "continuation" in a wire protocol isn't a new channel -- it's a state transition in the protocol state machine that both sides track. Par's mechanism and pane's transport are architecturally incompatible at the implementation level, even though they encode the same abstract session type.

This means pane can use par for two things: (a) defining session types as specifications, and (b) in-memory testing. It cannot use par for actual wire communication. The research document's "Phase 2 target" -- "Par session types driven over unix sockets" -- requires building a bridge layer that essentially reimplements par's operational semantics over a different substrate. At that point, you're not using par; you're using par's type definitions and reimplementing everything else.

**Crash handling: par panics on drop.** The code is explicit:

```rust
self.tx.send(Exchange::Send((value, dual)))
    .ok()
    .expect("receiver dropped")  // <-- this is a panic
```

When a client crashes and its session endpoints are dropped, the server side panics on the next operation. Par provides `#[must_use]` annotations for compiler warnings, but no mechanism to handle dropped endpoints gracefully.

For a compositor, this is disqualifying in production. If a misbehaving Electron app crashes, the compositor cannot panic. Haiku's app_server solved this at the kernel level: `set_port_owner()` transferred port ownership to the client team, so when the client team died, the kernel deleted the port, and `GetNextMessage()` returned `B_BAD_PORT_ID` -- an error code, not a crash. The server thread saw the error, sent `AS_DELETE_APP` to the Desktop thread, and cleaned up. The compositor continued.

Pane needs the same property: client death produces a typed event, not a panic. Par's architecture makes this impossible without wrapping every session operation in `catch_unwind`, which is ugly, has overhead, and doesn't compose with par's async model cleanly. `catch_unwind` across `.await` boundaries is undefined behavior in many async runtimes, and even where it works, it poisons the executor's state.

**fork_sync is blocking.** Par's cut rule implementation:

```rust
fn fork_sync(f: impl FnOnce(Self::Dual)) -> Self {
    let (recv, send) = endpoints();
    f(send);  // <-- blocks until f completes
    recv
}
```

The closure runs synchronously to completion before `fork_sync` returns. This is correct from the linear logic perspective (cut elimination produces a complete derivation), but it creates a tension with calloop.

The compositor's main thread is a calloop event loop. It cannot block on `fork_sync`. If `fork_sync` spawns a closure that does I/O or waits for a client, the main thread stalls. The architecture spec acknowledges this: "calloop's futures executor can drive par futures, but fork_sync needs careful handling."

"Careful handling" means: never call `fork_sync` from the calloop thread. Always spawn the forked work on a separate thread. This is doable but it means par's natural programming model (fork_sync as the primary composition mechanism) is fighting the compositor's concurrency model.

**Server module scope restrictions vs. per-pane threading.** Par's server module enforces that Server, Proxy, and Connection never coexist in the same scope, using closure-based APIs. The rationale is deadlock freedom. But pane's per-pane threading model means a pane thread needs to hold a Connection (to resume its session) while the main compositor thread holds the Server (to poll for events). These are different threads, different scopes -- but the scope restriction is enforced syntactically (closures), not by thread identity.

This may be workable. But it means par's Server module's ergonomics don't match pane's threading model naturally. The dispatcher thread (one per connection) that demuxes to per-pane threads would need to carefully thread Connection handles through to the right pane threads without violating par's scoping rules. This is solvable but is friction between par's model and pane's model.

### Net assessment of par

Par is a beautiful library that solves a different problem than the one pane has. Par solves: "how do I get session-type guarantees for in-memory concurrent Rust programs?" Pane's problem is: "how do I get session-type guarantees for processes communicating over serialized unix socket protocols, with crash resilience, integrated with a calloop event loop?"

Pane would use par's type definitions (the Session trait, Send/Recv as types) and throw away par's runtime (the oneshot channels, the fork_sync mechanism, the async execution model). At that point, the value of depending on par vs. defining equivalent types locally is marginal.

---

## 2. dialectic: Strengths and limitations

### What dialectic gets right

Dialectic solves the exact problem par doesn't: transport polymorphism. Its `Transmit`/`Receive` backend traits abstract over the transport. The `Chan<S, Tx, Rx>` type is parameterized by session type AND transport backend. This means you can define a session type once and run it over in-memory channels for testing and unix sockets for production. This is precisely what pane needs.

Dialectic's error handling is also superior for pane's use case. From the docs: it "gracefully handles runtime protocol violations, introducing no panics." Protocol violations return `Result` types (`SessionIncomplete`, `Unavailable`, `IncompleteHalf`) rather than panicking. Dropped endpoints produce errors, not crashes. This is the compositor's requirement.

The `Split` primitive (decompose a channel into send-only and receive-only halves) enables full-duplex concurrent communication, which maps to pane's async-by-default model where the client fires off drawing commands without waiting for responses.

The `Call` primitive (subroutine sessions) and `Loop`/`Continue` (type-level recursion with de Bruijn indices) are more expressive than par's approach.

### What dialectic gets wrong for pane

**Async dependency.** Dialectic is built on async/await. The reference backend implementations use Tokio. The core doesn't technically require Tokio, but the entire API surface is async. Pane's compositor uses calloop, not an async runtime. Pane's non-compositor servers use `std::thread` + channels, not async.

The architecture spec is explicit: "No async runtime. No system-wide executor. Just threads and channels." Dialectic's async-everywhere model is the opposite of this commitment. You could drive dialectic's futures from calloop's executor, but you'd be doing the same awkward bridging that par requires, just with a different async library underneath.

**Macro-heavy API.** Dialectic uses a `Session!` macro DSL for defining session types. This is ergonomic for reading but introduces a layer of indirection between what you write and what the compiler sees. When session type errors occur, you debug through macro expansion. For a project that wants to formally verify its session types in a proof assistant, having the session type definition be a proc-macro output rather than a directly inspectable type is a problem.

**Maintenance risk.** Dialectic is a Bolt Labs project. It has 63 stars and 3 forks. The last substantive development appears to have been some time ago. For a project that pane would depend on as foundational infrastructure, the maintenance trajectory matters. If Bolt Labs moves on, pane owns the dependency.

**Abstraction weight.** Dialectic's generality (context-free sessions, multiple calling conventions for Transmit, attribute macros for reducing trait bound boilerplate) is weight that pane doesn't need. Pane has one transport (unix sockets), one serialization format (postcard), and one wire protocol per server. The generality that makes dialectic powerful for arbitrary networked services is overhead for a desktop environment with a fixed, small set of protocols.

### Net assessment of dialectic

Dialectic proves the transport bridge is solvable and the error-handling-not-panics approach works. These are important existence proofs. But adopting dialectic wholesale means: accepting an async dependency the architecture explicitly rejects, depending on a library with uncertain maintenance, and carrying abstraction weight for generality pane doesn't need.

The right move is to learn from dialectic's design (backend traits, error handling, Split) without depending on it.

---

## 3. The case for a custom implementation

Here's what a custom implementation designed for pane would look like, and why the designer's type theory background makes this tractable rather than reckless.

### What custom buys you

**Transport-native from the ground up.** A custom session type library designed for pane doesn't need transport polymorphism. It needs exactly one thing: session types over unix sockets with postcard serialization. The session type advances by tracking protocol state on both sides, not by passing continuation channels through the transport. The implementation is a typestate machine:

```rust
// Conceptual sketch
struct Chan<S: Protocol, T: Transport> {
    transport: T,
    _state: PhantomData<S>,
}

impl<T: Transport> Chan<Send<Msg, Next>, T> {
    fn send(self, msg: Msg) -> Result<Chan<Next, T>, SessionError> {
        self.transport.write(&postcard::to_allocvec(&msg)?)?;
        Ok(Chan { transport: self.transport, _state: PhantomData })
    }
}
```

No oneshot channels. No continuation passing. The transport is a unix socket. The session type is a phantom type that advances with each operation. The `Chan` is consumed on each send/recv (enforcing linearity through ownership), and a new `Chan` at the next protocol state is returned. This is the typestate pattern, and it's the natural Rust encoding of session types for wire protocols.

**Crash handling as first-class concern.** Every operation returns `Result<NextState, SessionError>` where `SessionError` includes `Disconnected`, `SerializationError`, `Timeout`. When a client crashes, the socket closes, the next `recv()` gets `EPIPE` or `ECONNRESET`, and the server gets `Err(SessionError::Disconnected)`. No panics. No `catch_unwind`. The error propagates through the normal Rust error handling path.

This maps directly to Haiku's pattern: `set_port_owner()` made port death visible as an error return from `GetNextMessage()`. Pane's equivalent: socket ownership means socket death is visible as an error return from `recv()`. The session type library handles this by construction, not as an afterthought.

Fowler, Lindley, Morris, and Decova's "Exceptional Asynchronous Session Types" (POPL 2019) provides the theoretical foundation: session types extended with a `cancel` primitive that allows any endpoint to signal session termination. The dual of a cancellable session includes a handler for the cancellation. This is exactly what pane needs: every protocol includes an implicit "the other side died" branch, and the type system ensures the handler exists.

**calloop integration without async.** The custom implementation doesn't need futures. It needs:

1. A `Chan` that wraps a unix socket fd
2. Registration of that fd with calloop as an `EventSource`
3. On readable: deserialize the message, advance the session state, dispatch to handler
4. On writable: serialize the next outgoing message, advance the session state

This is callback-driven, not future-driven. It matches calloop's model exactly. No executor, no polling, no bridging between async and sync worlds.

On the client side (pane-app kit), the same `Chan` wraps a socket fd. The looper thread reads from the channel, dispatches messages, and the session type tracks where the conversation is. `std::thread` + `Chan<S, UnixSocket>` is the entire concurrency model.

**Dynamic optic composition support.** Par and dialectic know nothing about optics. The scripting protocol needs session types that can carry optic specifiers and resolve them at runtime. A custom implementation can define:

```rust
// Conceptual
enum ScriptingStep<S: Protocol> {
    Resolve(Specifier, Chan<ScriptingStep<S>>),  // peel and forward
    Execute(OpticAccess, Chan<ScriptingResult>),  // terminal: run the access
    NotFound(Chan<End>),                          // specifier didn't resolve
}
```

The session type for each step is known statically. The chain length and specific optics are dynamic. This is the controlled runtime dynamism that the architecture spec describes, but built into the session type library rather than bolted on top.

**Per-pane threading as a design primitive.** The custom implementation can be designed around pane's specific threading model: one `Chan` per pane, owned by the pane thread, with the dispatcher thread doing the initial demux. The type system can enforce that a `Chan` is used from exactly one thread (no `Send` bound -- the channel stays on its owning thread) while the transport socket can be registered with any event loop. This matches the ServerWindow/MessageLooper model exactly.

### What custom gives up

**Proven correctness.** Par implements the Wadler CP correspondence correctly. A custom implementation needs to be correct from scratch. The session type primitives (Send, Recv, Choose, Offer, End, recursion) need to enforce duality, prevent misuse, and preserve the session fidelity property. This is a non-trivial type-level programming exercise in Rust.

However: the core primitives are well-understood. The typestate pattern for session types in Rust has been explored by multiple implementations. The theory is 30 years old. The risk is not "is this possible?" but "will the implementation be correct?" -- which is where formal verification enters.

**Community and documentation.** Par has docs, examples, and a small community. A custom implementation starts at zero. For an open-source project that needs contributors, this matters. On the other hand, pane's session type library is a small, focused piece of infrastructure (not a general-purpose library), and its documentation can be integrated with pane's overall docs.

**Maintenance burden.** Every line of custom code is code pane maintains. Par's bugs are someone else's problem. But par's limitations are also someone else's problem -- and the limitations (no transport, panics on drop, blocking fork_sync) are exactly the problems pane needs solved.

---

## 4. The case against a custom implementation

I want to be honest about this because I've seen this movie before. At Be, we wrote everything from scratch because we could, because the team was extraordinary, and because vertical integration was the whole point. It worked -- until it didn't. The things that killed Be weren't technical. But the things that slowed us down were often "we built this ourselves when we could have used something that existed."

The risk of a custom session type library is scope creep. The designer starts with typestate-pattern session types over unix sockets. Then they need recursion, which means recursive types with proper coinductive treatment. Then they need the server module equivalent for multi-client. Then they need the queue/streaming abstraction. Then they realize the error handling interacts with branching in a subtle way and need to redesign the Choose/Offer mechanism. Each step is individually justified. The aggregate effect is that the session type library becomes its own research project, and Phase 2 stretches from weeks to months.

The Be engineers would ask: "Does this help us ship faster?" And the honest answer is: in the short term, no. In the medium term, if done right, yes -- because every protocol built on a sound foundation is cheaper to build and debug. But "if done right" is doing a lot of work in that sentence.

The mitigation is scope discipline. The custom implementation needs to be minimal:

1. `Chan<S, Transport>` with typestate advancement -- the core
2. `Send`, `Recv`, `Choose`, `Offer`, `End` -- the primitives
3. `Result`-based error handling with `SessionError` -- crash resilience
4. Duality derivation (probably via a proc macro or manual impl) -- the guarantee
5. A `UnixSocketTransport` using postcard -- the one backend

That's it. No transport polymorphism. No generic executor integration. No queue module. No server module. Those can come later if needed. The initial implementation is ~500-1000 lines of focused Rust.

---

## 5. The formal verification angle

The designer mentions verifying the session types in a dependent type theory. This is where I need to separate three things that are easy to conflate:

### (a) Verifying the primitives

Prove in Agda/Lean/Coq that the session type primitives (Send, Recv, Choose, Offer, End, duality, composition) satisfy session fidelity, communication safety, and the relevant progress property. This is:

- **Tractable.** The theory is well-worked-out. Caires-Pfenning, Wadler, and subsequent work provide the proof strategies. A type theorist with domain expertise can formalize the core in weeks, not months.
- **High-value.** It gives pane a verified foundation that par doesn't have (par cites the theory but doesn't mechanize the proofs). It's a credibility signal: "our protocol infrastructure is formally verified" is a statement that very few systems projects can make.
- **Incremental.** Verify the primitives first. The compositions follow from the primitive proofs. You don't need to verify every specific protocol -- you need to verify that the composition mechanism preserves the invariants.

### (b) Verifying specific protocols

Prove that the PaneSession, CompSession, RosterSession, etc. satisfy their intended properties. This is more work but also more incremental -- each protocol is a separate proof that reuses the primitive verification.

### (c) Verifying the Rust implementation matches the formal model

Prove that the Rust code correctly implements what the formal model describes. This is the hardest step and the one most likely to be a time sink. Rust doesn't have dependent types. The gap between the Agda/Lean model and the Rust implementation must be bridged by argument, not by proof. Tools like Prusti, Creusot, or Kani can help verify specific properties of the Rust code, but full correspondence between the formal model and the implementation is a research problem.

### My recommendation

Do (a) now, (b) incrementally as protocols are defined, and (c) later or never. The value curve drops off sharply: (a) is cheap and high-value, (b) is moderate cost and moderate value, (c) is expensive and its value depends on whether pane develops a security-critical use case that demands it.

The formal verification is not a time sink if scoped to (a). It becomes one if it expands to (c) before the desktop ships.

Specifically: the designer should write the session type primitives in Lean or Agda alongside the Rust implementation, proving session fidelity and duality correctness for the primitives. This is a 2-3 week investment that produces a verified core. Every protocol built on that core inherits the verification by construction (assuming the Rust implementation faithfully implements the model -- which is the (c) gap, but one that can be managed by keeping the Rust implementation structurally close to the formal model).

---

## 6. Recommendation

Build a minimal custom implementation. Here's why, in order of importance:

### The transport bridge is the project's critical path

Phase 2 is the transport bridge prototype. Par doesn't solve it. Dialectic solves it but brings an async dependency the architecture rejects. A custom implementation solves it directly because it's designed for the specific transport from the start.

The alternative -- "use par for specification, build a hand-written state machine for the wire" -- is the fallback the research document describes. But it means the wire transport doesn't get session type enforcement. The protocol ordering is verified "by developer discipline, not the compiler." The research document correctly notes this is "the same assurance level BeOS had." That's adequate. But the whole point of session types is to do better than adequate.

A custom typestate implementation over unix sockets gives session type enforcement on the wire, not just in tests. The `Chan<S, UnixSocket>` enforces protocol ordering at compile time for the actual production transport, not just for in-memory test doubles. This is the difference between "the session type is the spec" (par approach) and "the session type is the implementation" (custom approach).

### Crash handling cannot be an afterthought

Par panics on drop. The architecture spec says "a dropped session endpoint produces a 'session terminated' event, not a panic." These are contradictory. Wrapping par operations in `catch_unwind` is not a principled solution -- it's error-prone, has undefined behavior with async, and doesn't compose.

A custom implementation with `Result`-based error handling solves this by construction. `recv()` returns `Result<(T, Chan<Next, Transport>), SessionError>`. A crashed client produces `Err(SessionError::Disconnected)`. The server handles it through normal Rust error handling. No panics, no `catch_unwind`, no special cases.

This is the lesson from Haiku's app_server. The crash handling wasn't a wrapper around the messaging system -- it was built into the messaging system. `GetNextMessage()` returned `status_t`, not `void`. When the port died, you got `B_BAD_PORT_ID`, not a crash. The same principle applies here: error handling must be in the type, not around it.

### The designer has the expertise

This is the factor that tips the scale. A custom session type library in Rust, designed for a specific use case, with a specific transport, is not a research project. It's an engineering project informed by well-understood theory. The primitives are known. The typestate pattern is known. The formal verification (scope (a)) is tractable for someone with type theory background.

If the designer were a systems programmer without type theory expertise, I'd say use par and live with the limitations. But the ability to design session types that natively handle error cases, verify them formally, and implement them idiomatically in Rust's type system -- that's a genuine advantage that shouldn't be left on the table.

### The scope must be ruthlessly constrained

The custom implementation for Phase 2 is:

1. **Protocol trait** -- the equivalent of par's `Session`, with `Dual` involution
2. **Typestate primitives** -- `Send<T, S>`, `Recv<T, S>`, `Choose<Options>`, `Offer<Options>`, `End`
3. **Chan<S, T>** -- a channel parameterized by session state and transport, consumed on each operation
4. **UnixSocketTransport** -- postcard serialization over a unix socket fd
5. **SessionError** -- `Disconnected`, `Serialization`, `Timeout`, `ProtocolViolation`
6. **Duality derivation** -- a derive macro or manual implementations
7. **calloop EventSource impl** -- register a `Chan` as a calloop event source for compositor-side use

That's the Phase 2 deliverable. No queue module. No server module. No transport polymorphism. No formal verification yet (that can parallel the implementation). The queue and server patterns come in Phase 3 when the multi-client compositor needs them.

### What to take from par and dialectic

- **From par:** The Session trait design (Dual involution, duality as associated type). Enum-based branching. The Queue abstraction (implement later). The theoretical grounding in Wadler's CP.
- **From dialectic:** `Result`-based error handling. The backend trait concept (but implement only one backend). `Split` for full-duplex. The existence proof that transport-polymorphic session types work.
- **From Fowler et al.:** Exceptional session types -- the theoretical basis for making cancellation/crash a first-class protocol concern rather than an out-of-band mechanism.

### What to avoid

- **Don't build a general-purpose session type library.** Build pane's session type library. It serves pane's specific needs. If it's useful to others later, great. But generality is not a goal.
- **Don't make the formal verification a prerequisite for Phase 2.** Verify in parallel. The Lean/Agda model and the Rust implementation can develop simultaneously. The model informs the implementation; the implementation tests the model's assumptions.
- **Don't abandon par for testing.** Par's in-memory channels are perfect for protocol testing. Define pane's session types, write property tests using par's in-memory execution, and use the custom transport for production. Par as test infrastructure, not production infrastructure.

Actually, strike that last point. Par's specific mechanism (oneshot continuation passing) is different enough from the custom typestate mechanism that tests written against par wouldn't test the same code paths as production. Better to write an `InMemoryTransport` backend for the custom implementation and test against that. The in-memory backend uses channels internally but presents the same `Chan<S, InMemoryTransport>` interface as the unix socket backend. Same types, same code paths, different substrate.

---

## 7. Timeline risk assessment

The Be engineer in me wants to say: "just build it and stop talking about it." But I've also seen what happens when Phase 2 becomes a research project.

**Best case (custom implementation, disciplined scope):** 3-4 weeks for the core (primitives, Chan, UnixSocketTransport, calloop integration, error handling). 1-2 weeks for the initial formal model in Lean/Agda (just the primitives). Phase 2 total: 5-6 weeks. This is longer than "just use par and build a bridge" (which is 2-3 weeks for a half-solution), but it produces a full solution that every subsequent phase builds on.

**Worst case (scope creep):** The designer gets pulled into making the session type library general-purpose, or the formal verification expands to cover protocol-specific properties before the protocols exist, or the transport abstraction becomes polymorphic "just in case." Phase 2 stretches to 3+ months and the desktop is still hypothetical.

**Mitigation:** The Phase 2 acceptance criteria should be: "pane-comp's calloop main thread communicates with a pane-shell client process over a unix socket, using session-typed `Chan` values, with crash recovery demonstrated by killing the client mid-session." That's the demo. When the demo works, Phase 2 is done. Everything else is Phase 3+.

---

## Summary

| Factor | par | dialectic | Custom |
|---|---|---|---|
| Transport bridge | Not solved | Solved (async) | Solved (sync, calloop-native) |
| Crash handling | Panics | Result-based | Result-based by design |
| calloop integration | Async bridge needed | Async bridge needed | Native fit |
| Per-pane threading | Scope restrictions fight it | Async model fights it | Designed for it |
| Scripting/optics | No support | No support | Can be designed in |
| Formal verification | Theory cited, not mechanized | Not applicable | Tractable for the designer |
| Maintenance burden | External dependency | External dependency (uncertain maintenance) | Owned by pane |
| Time to Phase 2 demo | 2-3 weeks (half-solution) | 3-4 weeks (wrong concurrency model) | 5-6 weeks (full solution) |
| Time to Phase 5 (daily driver) | Accumulating bridge debt | Accumulating async/sync friction | Clean path |

The recommendation is: custom implementation, minimal scope, formal verification of primitives in parallel, acceptance criteria defined by a working demo, not by theoretical completeness.

BeOS shipped because it worked, not because it was perfect. The session type library needs to work for Phase 2's demo. It can be perfected in the phases that follow. But it needs to be designed right from the start -- the transport, the error handling, and the calloop integration are load-bearing decisions that compound through every subsequent phase. Getting these wrong with par and fixing them later costs more than getting them right with a custom implementation now.

---

## Sources

### Assessed libraries
- faiface/par v0.3.10 -- source read: lib.rs, exchange.rs, server.rs, queue.rs. <https://github.com/faiface/par>
- boltlabs-inc/dialectic -- API docs and backend trait design reviewed. <https://github.com/boltlabs-inc/dialectic>

### Theoretical foundations
- Wadler, "Propositions as Sessions" (ICFP 2012 / JFP 2014) -- par's theoretical basis
- Fowler, Lindley, Morris, Decova, "Exceptional Asynchronous Session Types" (POPL 2019) -- session types with crash/cancellation handling
- Caires, Pfenning, "Session Types as Intuitionistic Linear Propositions" (CONCUR 2010)

### Haiku reference implementation
- `src/servers/app/ServerApp.cpp` lines 129-134: `set_port_owner()` transfers port to client team, making crash visible as error return from `GetNextMessage()`. This is the BeOS crash isolation pattern.
- `src/servers/app/MessageLooper.cpp` lines 140-164: the message loop returns on port error rather than crashing. Error is an event, not an exception.
- `src/servers/app/ServerWindow.h`: ServerWindow inherits MessageLooper, one thread per window.

### Typestate pattern in Rust
- The typestate pattern (consuming `self`, returning a value at the next state type) is the standard Rust encoding of session types for wire protocols. It's used by tower-sessions, tungstenite's handshake, and numerous protocol implementations. It doesn't require a library -- it's idiomatic Rust.
