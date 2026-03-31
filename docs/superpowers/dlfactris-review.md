# DLfActRiS Review: Relevance to Pane

Paper: Jacobs, Hinrichsen, Krebbers. "Deadlock-Free Separation Logic: Linearity Yields Progress for Dependent Higher-Order Message Passing." POPL 2024. https://doi.org/10.1145/3632889

Reviewed against: pane-session, pane-app, architecture.md §5 (Scripting Protocol), §13 (Open Questions: affine/linear gap, dynamic optic composition).

---

## What the paper does

LinearActris is a *linear* concurrent separation logic built on top of Iris. Its central theorem: if you can prove `Emp ⊢ WP e {Emp}` (the program starts with nothing and returns nothing), then `e` enjoys **global progress** — no deadlocks, no leaks, all channels eventually deallocated. The guarantee comes "for free" from linearity. No lock orders, no priority annotations, no acyclicity obligations on the programmer.

The logic reasons about programs in ChanLang — a concurrent lambda calculus with `fork`, `send`, `recv`, `close`, `wait`, mutable references, and closures as first-class values. Protocols are *dependent separation protocols* in the Actris style: they can express logical conditions on messages (not just types), transfer ownership of heap resources alongside messages, and use quantifiers to abstract over values.

The adequacy proof works by maintaining an invariant over *connectivity graphs* — directed graphs where nodes are threads and channels, edges represent ownership, and strong acyclicity (no undirected cycles) is preserved through every operation. This is the key technical device. Each channel operation (fork, send, recv) transforms the graph in a way that preserves acyclicity, which rules out deadlocks (cyclic waits) and leaks (cyclic ownership preventing deallocation).

---

## The three properties that make it work

LinearActris needs three things to guarantee deadlock and leak freedom:

**1. Linearity (not affinity).** Resources must be used exactly once — you cannot silently drop a channel endpoint. This is the critical upgrade from Iris/Actris, which are *affine* (resources can be dropped). The paper shows (p. 47:3) that affinity allows a thread to drop `c₂` while the peer blocks forever on `recv(c₁)`. Linearity forces you to close or wait on every channel you create.

**2. Acyclicity of channel ownership.** No thread can hold both endpoints of the same channel (weak acyclicity). More generally, the connectivity graph of channel ownership assertions must be strongly acyclic — no undirected cycles even when traversing edges in either direction. This prevents the classic two-thread deadlock where Thread 1 holds `recv c₁; send d₁` and Thread 2 holds `recv d₂; send c₂`.

**3. Channel fulfillment.** Terminated threads must not hold channel ownership assertions. If a thread finishes while still "owing" a send on some channel, the peer blocks forever. Linearity enforces this: you can't let a resource go unused.

---

## Direct relevance to pane

### The affine/linear gap (architecture §13, open question)

This is the paper's primary point of contact with pane. The architecture doc flags this explicitly:

> "Rust's `#[must_use]` generates warnings for dropped session endpoints, not hard errors. A crashed process drops endpoints silently."

Pane's `Chan<S, T>` is `#[must_use]`, but Rust's ownership system is *affine*, not linear. You can forget to use a `Chan`, and the compiler won't stop you — it'll warn, but the code compiles. A panicking thread drops all its locals, including any `Chan` it holds, leaving the peer blocked forever on a `recv` that will never arrive.

LinearActris tells us exactly what goes wrong and why: affinity lets you violate the *channel fulfillment* property. A dropped send-endpoint is a thread that "terminates" while still owing a message. The paper's Theorem 1.2 (global progress) specifically requires the heap to be empty at termination — all channels deallocated, all obligations met.

**What pane currently does about this:** The crash boundary. `SessionError::Disconnected` converts a dropped channel into an error on the peer side rather than an indefinite block. The `ReplyPort` Drop impl sends `ReplyFailed` when dropped without replying. These are runtime mitigations for the static gap that LinearActris identifies.

**What the paper suggests pane could do:**

The paper doesn't solve this for Rust (its proofs are in Coq over ChanLang), but it clarifies the *shape* of the solution. There are three paths:

(a) **Accept affinity + runtime recovery (current approach).** Pane already does this. The paper validates that the recovery is *necessary* — without linearity, deadlock freedom doesn't come for free. The cost is that pane's session types guarantee protocol adherence but not deadlock freedom. The architecture doc's crash-safety design (`Disconnected` errors, `ReplyFailed` on drop) is the right engineering response to this theoretical gap.

(b) **Encode linearity via Ferrite's approach.** Ferrite (Chen, Balzer, Toninho, ECOOP 2022) embeds linear session types in Rust using a judgmental embedding — a continuation-passing style where the session continuation is *inside* the API, not returned to the caller. This prevents the caller from dropping it. The trade-off is a very different API shape: instead of `let chan = chan.send(x)?; let (y, chan) = chan.recv()?`, you'd write something like `session.send(x, |session| session.recv(|y, session| ...))`. This is a fundamental API redesign and may conflict with pane's goal of keeping the kit simple and BeOS-familiar.

(c) **Runtime leak/deadlock detection using connectivity graphs.** The paper's connectivity graph invariant could be implemented as a runtime debug tool. Track channel ownership in a directed graph; check for cycles periodically or on thread exit. This wouldn't prevent deadlocks but would *detect* them immediately with a clear diagnostic ("thread T₁ blocked on channel C₁, which forms a cycle with T₂ via channel C₂"). This is the most practical near-term application: a debug mode that validates the connectivity graph invariant at runtime.

### Dependent separation protocols and the scripting protocol

LinearActris's protocols are more expressive than pane's current session types in a way that matters for the scripting protocol (architecture §5).

Pane's `Chan<Send<A, S>, T>` says "send a value of type A, then continue as S." The type `A` is fixed — it's a Rust type determined at compile time.

LinearActris's `!(x⃗)(v){P}; p` says "send a value `v` where there exist mathematical variables `x⃗` such that `v` equals some term, *and* transfer ownership of the resources described by separation logic proposition `P`, and the continuation protocol `p` can depend on `x⃗`."

The difference: LinearActris protocols carry *logical conditions* alongside messages, and the continuation can depend on the *value* sent. This is exactly what pane's scripting protocol needs.

Consider the scripting interaction "get property X of pane P." The response protocol depends on *which* property X is — a title query returns a string with a string-shaped continuation, while a geometry query returns dimensions. In pane's current session types, you'd need to enumerate all possible queries as a `Select` tree. In LinearActris's dependent protocols, the continuation is a function of the query value.

This maps to the architecture doc's description: "The session type for each step is known statically (it's always 'send specifier, receive result or forward'), but the chain length and specific optics are dynamic." LinearActris's quantified protocols are the formal mechanism for this — the session type *shape* is static (send query, receive result), but the protocol *content* (which property, what type of result) is parameterized.

**Practical implication:** Pane can't directly use dependent separation protocols (they live in Coq). But the *pattern* — protocols whose continuations branch on runtime values — can be approximated in Rust using enum dispatch within a session step. Each step of the scripting protocol would be a `Send<ScriptQuery, Branch<...>>` where the branch is determined by the query. The key insight from the paper is that this is *sound* — dependent branching doesn't break deadlock freedom as long as the acyclicity invariant holds.

### Resource transfer and optics

LinearActris protocols can transfer *ownership of heap resources* alongside messages. The notation `{ℓ ↦ n}` in a protocol step means "this send also transfers ownership of location ℓ holding value n."

This connects to the optics discussion. When a pane sets a property through the scripting protocol, it's not just sending a value — it's transferring *authority over a piece of state*. A lens `set` operation on pane state changes who "owns" that slice of state during the operation. LinearActris gives a formal language for expressing this: the protocol can say "send the new title value, and receive back ownership of the title state updated to the new value."

The GetPut / PutGet laws from optics map to specific protocol assertions in LinearActris:

- **GetPut**: `send(get(s)); recv(s')` where `s' = s` — reading and writing back the same value is identity
- **PutGet**: `send(v); recv(s')` then `send(get(s')); recv(v')` where `v' = v` — what you put is what you get

These are exactly the kind of logical conditions that dependent separation protocols can express. The protocol for a property access could carry the assertion that the optic laws hold.

### Subprotocols and the specifier chain problem

LinearActris supports *subprotocols* — a refinement relation `p₁ ⊑ p₂` meaning "anywhere `p₂` is expected, `p₁` can be used." This is analogous to subtyping for protocols.

This is relevant to the dynamic specifier chain problem. When a scripting query traverses multiple handlers (like BeOS's `ResolveSpecifier`), each handler "refines" the protocol by resolving one step. The handler receives a generic "query" protocol and returns a more specific "result" protocol. Subprotocols provide the formal relationship: the unresolved protocol is a subprotocol of the fully-resolved one, and each resolution step is a valid subprotocol refinement.

The paper's subprotocol rules (Fig. 3, p. 47:10) show how to specialize quantifiers, strengthen transferred resources, and refine continuations — all of which would be needed for composable specifier resolution.

### Higher-order channels and the observer pattern

LinearActris can verify programs that send *channels over channels* and *closures over channels*. The paper demonstrates (§2, p. 47:7-8) that sending a channel `c₁` over channel `d₁` is safe as long as the ownership transfer is reflected in the connectivity graph — the sender gives up ownership of `c₁`'s endpoint, and the receiver gains it.

This is directly relevant to two pane features:

**Observer pattern (architecture §5, API Tier 2).** When pane implements `Messenger::start_watching(property, watcher)`, the watcher registration is establishing a new communication relationship. In LinearActris terms, the watcher sends a channel endpoint to the observed pane, creating a new edge in the connectivity graph. The paper's proof that this preserves acyclicity (as long as channels aren't sent circularly) validates the design pattern.

**Inter-pane request forwarding.** When a scripting query is forwarded from one handler to another (the `ResolveSpecifier` chain), the forwarding pane passes the reply channel to the next handler. This is sending a channel over a channel. LinearActris's proof rules for this pattern (WP-SEND rule, Fig. 3) show exactly what ownership transfer is required to maintain deadlock freedom.

### The fork-join pattern and pane threading

ChanLang's `fork` operation creates a new thread *and* a new channel connecting it to the parent. LinearActris proves that this combined operation preserves connectivity graph acyclicity. Separating thread creation from channel creation would break this — you'd need additional mechanisms to maintain acyclicity.

Pane's current model is different: threads (panes) and channels are created separately. `App::connect()` creates the connection, then `Pane::run()` starts the thread. The channel exists before the thread does. This is fine for safety (the channel endpoints are owned correctly), but it means pane can't directly appeal to LinearActris's fork-based acyclicity argument for deadlock freedom.

However, the `Pane::run` + `Messenger` pattern is structurally similar: each pane gets exactly one connection to the compositor, the pane thread owns one endpoint, and the dispatcher thread owns the other. This is a fork topology — tree-structured, no cycles. The paper confirms that this topology is inherently deadlock-free for the compositor-to-pane communication pattern.

The risk comes with *inter-pane* communication (API Tier 2). When pane A sends a request to pane B via `send_request`, and B might send back to A, you get the two-thread cross-wait pattern that LinearActris identifies as requiring acyclicity checking. Pane's current `send_and_wait` blocks on a one-shot channel, which creates a temporary cycle if both panes are waiting on each other — exactly the deadlock the paper's connectivity graph catches. The existing `WouldDeadlock` guard (rejecting `send_and_wait` from looper threads) is a coarse but effective mitigation.

---

## Concrete recommendations

### Near-term (Phase 4-5, no architectural change)

**1. Add a debug-mode connectivity graph tracker.** Instrument channel creation, send, recv, close, and thread exit to maintain a runtime connectivity graph. On thread exit, assert that no channel ownership assertions remain (channel fulfillment). Periodically (or on `send_and_wait` timeout) check for cycles. This gives LinearActris-grade diagnostics without requiring a proof assistant.

**2. Document the linearity gap explicitly.** The architecture doc §13 should reference this paper by name. The gap is well-characterized now: pane's session types are *affine* (Rust's ownership model), not *linear* (LinearActris's model). Deadlock freedom does not come for free. Pane's strategy is runtime crash recovery (`Disconnected`, `ReplyFailed`), and this is a principled engineering choice, not a hack.

**3. Strengthen `ReplyPort`'s linearity story.** `ReplyPort` already implements drop-as-failure, which is the LinearActris pattern for handling non-linear endpoints. Consider whether `Chan<S, T>` itself should do the same — when a `Chan<Send<A, S>, T>` is dropped without calling `send()`, the transport could be explicitly closed with an error, unblocking the peer immediately rather than waiting for the OS to tear down the socket.

### Medium-term (Phase 6, scripting protocol)

**4. Use dependent-protocol-inspired design for scripting sessions.** The scripting protocol session type should be parameterized on the query: the continuation (what you receive back) depends on what you asked for. In Rust, this maps to a `Send<ScriptQuery, Recv<ScriptResult, ...>>` where `ScriptResult` is an enum dispatched on the query variant. The paper validates that this pattern is sound for deadlock freedom.

**5. Model specifier chain forwarding as channel-over-channel passing.** When handler A forwards a scripting query to handler B, model this explicitly as A sending B the reply channel. LinearActris's WP-SEND rule for channel transfer shows the ownership transfer that makes this safe. This gives a formal basis for the "peel, resolve, forward" pattern.

### Long-term (research direction)

**6. Investigate Ferrite-style linearity for critical sub-protocols.** For the highest-assurance protocol paths (compositor handshake, crash recovery), consider whether Ferrite's judgmental embedding could provide stronger guarantees than `#[must_use]`. This would be a targeted application — not replacing `Chan<S, T>` wholesale, but wrapping specific critical protocols in a linear API.

**7. Connectivity graphs as a design tool for inter-pane protocols.** Before implementing API Tier 2 (clipboard, observer, drag-and-drop), sketch the connectivity graph for each new protocol relationship. If adding a new channel type creates the possibility of undirected cycles in the ownership graph, that protocol needs a deadlock mitigation strategy. This is a design-time discipline, not a verification tool, but it operationalizes the paper's central insight.

---

## What the paper doesn't address (gaps relative to pane)

**Crash recovery.** LinearActris assumes clean termination. Pane's reality is that processes crash — `SIGKILL`, OOM, segfault. The paper's adequacy theorem says nothing about what happens when a thread disappears without executing its cleanup. Pane's `Disconnected` error path is entirely outside LinearActris's model. This is the biggest gap: the paper proves deadlock freedom for well-behaved programs, but pane needs resilience against ill-behaved ones.

**Asynchronous subtyping.** LinearActris does not support asynchronous subtyping (where a sender can be ahead of the receiver by buffered messages). The paper acknowledges this limitation (§10). Pane's bounded channels (256-message buffer) allow some degree of asynchronous communication that LinearActris cannot reason about.

**Multi-party protocols.** LinearActris handles binary (two-party) channels. Pane's compositor protocol is inherently multi-party: the compositor talks to N panes, and panes may need to coordinate with each other. The paper's §10 mentions extending to multiparty as future work. For pane, multiparty session types (Honda et al. 2008, Scalas and Yoshida 2019) remain more directly relevant for the N-pane coordination problem.

**Liveness.** LinearActris guarantees deadlock freedom (a safety property: no stuck state) but not liveness (a liveness property: eventual progress). A pane that enters an infinite loop without touching any channel is "deadlock-free" by the paper's definition but completely unresponsive. Liveness requires separate machinery (the paper cites LiLi and TaDa Live as future directions).

---

## Summary assessment

Relevance to pane: **high for understanding, moderate for direct application.**

The paper's biggest contribution to pane is *naming the problem precisely*. The affine/linear gap, which the architecture doc identified as an open question, is exactly the gap between Iris (affine) and LinearActris (linear). The paper proves that linearity is *necessary* for deadlock freedom from types alone — you can't get it from affinity. This validates pane's engineering choice to handle the gap at runtime (crash recovery, `ReplyFailed` on drop) rather than trying to achieve it statically in Rust's type system.

The dependent separation protocols and connectivity graphs are intellectually valuable as design tools for the scripting protocol and inter-pane communication, even though they can't be directly embedded in Rust code. The connectivity graph in particular is something that could be implemented as a runtime debugging aid.

The paper does not supersede pane's existing session type design. Pane's `Chan<S, T>` with `HasDual` is a sound, practical embedding of binary session types in Rust. LinearActris operates at a different level — it's a *verification logic* for proving properties about programs that use session-typed channels, not a replacement for the channels themselves.

---

Sources:
- Jacobs, Hinrichsen, Krebbers. POPL 2024. https://doi.org/10.1145/3632889
- Coq mechanization: https://doi.org/10.5281/zenodo.8422755
- Actris 2.0 (predecessor): Hinrichsen et al. 2022. https://doi.org/10.46298/lmcs-18(2:16)2022
- Ferrite: Chen, Balzer, Toninho. ECOOP 2022. https://arxiv.org/abs/2205.06921
- Connectivity graphs: Jacobs et al. 2022a. https://doi.org/10.1145/3498662
