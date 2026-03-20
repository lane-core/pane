# Monadic Error Handling in Session-Typed Message-Passing Systems

Research for pane spec-tightening. Covers monadic/compositional error handling patterns and how they apply to a system of communicating components connected by session-typed protocols, where crashes must not cascade.

Sources:

- Fowler, Lindley, Morris, Decova. ["Exceptional Asynchronous Session Types: Session Types without Tiers."](https://dl.acm.org/doi/10.1145/3290341) POPL 2019.
- Lagaillardie, Neykova, Yoshida. ["Stay Safe Under Panic: Affine Rust Programming with Multiparty Session Types."](https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.ECOOP.2022.4) ECOOP 2022.
- Barwell, Scalas, Yoshida, Zhou. ["Generalised Multiparty Session Types with Crash-Stop Failures."](https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.CONCUR.2022.35) CONCUR 2022.
- Wadler. ["Propositions as Sessions."](https://homepages.inf.ed.ac.uk/wadler/papers/propositions-as-sessions/propositions-as-sessions.pdf) ICFP 2012 / JFP 2014.
- Caires, Pfenning. ["Linear Logic Propositions as Session Types."](https://www.cs.cmu.edu/~fp/papers/mscs13.pdf) MSCS 2013.
- faiface/par. ["Session types for Rust."](https://github.com/faiface/par) v0.3.10.
- Aria (Faultlore). ["The Pain of Linear Types in Rust."](https://faultlore.com/blah/linear-rust/)
- Yoshua Wuyts. ["Linearity and Control."](https://blog.yoshuawuyts.com/linearity-and-control)
- Erlang/OTP documentation: [Supervision Trees.](https://adoptingerlang.org/docs/development/supervision_trees/)
- Akka documentation: [Supervision and Monitoring.](https://doc.akka.io/libraries/akka-core/current/general/supervision.html)
- Bogard. ["Functional Error Handling with Monads, Monad Transformers and Cats MTL."](https://guillaumebogard.dev/posts/functional-error-handling/)
- Lindig. ["Monadic Error Handling."](https://medium.com/@huund/monadic-error-handling-1e2ce66e3810)
- Rust std::panic::catch_unwind [documentation.](https://doc.rust-lang.org/std/panic/fn.catch_unwind.html)
- Haiku launch_daemon documentation and BRoster API (haiku-os.org).
- Be Newsletter archives, specifically Pavel Cisler on thread synchronization (Issue 3-33), George Hoffman on app_server threading (Issue 2-36).

---

## 1. Monadic Error Handling Fundamentals

### The Error monad is the compositional alternative to exceptions

The core insight of monadic error handling: errors are values, not control flow. Instead of throwing an exception that unwinds the stack searching for a handler, a function that can fail returns `Result<T, E>` (Rust) or `Either<E, A>` (Haskell). The error is data. It participates in the type system. The compiler enforces that callers acknowledge it.

The monadic interface over `Result` is the `and_then` operation (known as `bind` or `>>=` in Haskell):

```rust
fn and_then<U, F>(self, f: F) -> Result<U, E>
where F: FnOnce(T) -> Result<U, E>
```

This is the composition operator. Given a `Result<T, E>` and a function `T -> Result<U, E>`, it:
- If `Ok(t)`: applies `f(t)`, returning its result
- If `Err(e)`: propagates `e` unchanged, skipping `f` entirely

A chain of `and_then` calls forms a pipeline where the first error short-circuits the rest. Rust's `?` operator is syntactic sugar for this pattern:

```rust
// These are equivalent:
let x = foo()?.bar()?.baz()?;

let x = foo()
    .and_then(|v| v.bar())
    .and_then(|v| v.baz());
```

### What monadic composition buys over exceptions

**Totality.** A function with signature `fn connect(addr: &str) -> Result<Connection, ConnectError>` declares its failure modes in the type. A function with signature `fn connect(addr: &str) -> Connection` that throws appears total but isn't. The monadic approach makes failure modes visible at every call site, enabling the compiler to enforce exhaustive handling.

**Compositionality.** Monadic bind is associative. Pipelines compose: if `f: A -> Result<B, E>` and `g: B -> Result<C, E>`, then `f >=> g: A -> Result<C, E>` (Kleisli composition). Error handling composes the same way the happy path does. No separate exception-handling mechanism is needed -- the same composition operator handles both cases.

**Referential transparency.** A `Result` value can be stored, passed to another function, retried, logged, or inspected. An exception cannot -- it exists only during stack unwinding. This matters for concurrent systems where an error in one thread must be communicated to another thread as a value, not as an unwinding stack frame.

**Type-level error channels.** Different error types can be composed. A function that calls both `connect()` (which returns `ConnectError`) and `parse()` (which returns `ParseError`) can return `Result<T, AppError>` where `AppError` is an enum encompassing both. The type system tracks which errors are possible at each point. This is the dual-channel pattern from Cats MTL: business errors (domain-specific, recoverable, actionable) separate from technical errors (infrastructure failures, requiring different recovery strategies).

### CPS and the connection to session types

Session types are continuation-based. After `Send<T, S>`, the continuation type is `S`. After `Recv<T, S>`, the continuation is `S`. The protocol IS a chain of continuations, each step's type determined by the previous step's output.

Monadic error handling over continuations is the `ContT` (continuation monad transformer) combined with `Either`. In CPS, a computation doesn't return a value -- it passes it to a continuation. An error-aware CPS computation passes either a value or an error to its continuation:

```
// Direct style:
fn step(x: A) -> Result<B, E>

// CPS style:
fn step_cps(x: A, ok: impl FnOnce(B), err: impl FnOnce(E))
```

The relevance to session types: a session type IS a typed continuation. `Send<T, S>` says "after sending T, the continuation has type S." If we want error handling in session types, we need error-aware continuations -- continuations that branch on success or failure. This is exactly what `Result` in a session type position achieves:

```rust
// A session step that can fail:
type FallibleStep = Send<Request, Recv<Result<Response, ServerError>, Continue>>;
```

The server sends back a `Result`. The client's continuation branches on the outcome. The error is part of the protocol, not an out-of-band exception.

---

## 2. Error Handling in Session-Typed Systems

### The problem: session types describe the happy path

A session type like `Send<Request, Recv<Response, Close>>` describes what happens when everything works. It says nothing about:

- What happens if the `Recv` blocks forever because the server crashed
- What happens if deserialization fails and the received bytes don't parse as `Response`
- What happens if the client panics after sending the request but before receiving the response

In the `par` crate's implementation, a dropped session endpoint causes the counterpart to panic. The `Recv` side panics with "sender dropped" when it tries to receive from a channel whose `Send` end was destroyed. This is the direct consequence of Rust's affine types: values CAN be dropped, and the session type system has no mechanism to prevent it.

For a desktop environment, this is not academic. A crashed client drops its `par` session endpoint. The compositor, blocked in a `recv()` on that client's session, panics. The compositor panics, every window disappears, the user loses their work. This is categorically unacceptable.

### Approach 1: Explicit error branches in the session type

The most direct approach: make errors part of the protocol.

```rust
enum ClientAction {
    WriteCells(Send<CellRegion, Recv<WriteResult, Recv<ClientAction>>>),
    SetTag(Send<TagLine, Recv<TagResult, Recv<ClientAction>>>),
    Close,
}

enum WriteResult {
    Ok(()),            // continue normally
    Err(Send<WriteError>),  // error reported, session continues
}
```

Every operation that can fail returns a `Result`-like choice. The client and server both handle both branches. The protocol is total -- it accounts for errors.

**Advantages:**
- Fully typed. The compiler enforces error handling on both sides.
- No special mechanisms needed -- just standard session type branching.
- Error recovery is part of the protocol. The session can continue after an error (retry, fallback, etc.).

**Disadvantages:**
- Verbose. Every operation needs a Result branch, doubling the protocol size.
- Does not handle crashes. If the client process dies, it doesn't send the error branch -- it drops the channel. The explicit error branch only helps for expected, within-protocol errors (invalid arguments, resource exhaustion, permission denied). It does not help for process death.
- Does not handle transport failures. Deserialization errors, socket disconnection -- these happen below the session type level.

This approach is correct for **application-level errors** -- the kind where both parties are alive and communicating, but the operation failed. It maps cleanly to Rust's `Result` type. The server sends `Result<T, E>` as the response, the client matches on it. This is the monadic error handling pattern lifted into the session type.

### Approach 2: Exceptional session types (Fowler et al.)

Fowler, Lindley, Morris, and Decova (POPL 2019) presented the first formal integration of asynchronous session types with exception handling. The key insight: when an exception is raised in a session-typed context, all open channels held by the failing process must be **cancelled**.

Their system extends session types with three constructs:

1. **`raise`**: raises an exception, cancelling all channels in scope
2. **`try M as x in N otherwise P`**: exception handler -- runs `M`, binds the result to `x` in `N`, catches exceptions in `P`
3. **`cancel c`**: explicitly cancels a channel endpoint `c`

Cancellation is the critical concept. When you cancel a channel:
- The other endpoint receives a cancellation signal
- If the other side is blocked on a `recv`, it gets a cancellation exception
- The cancellation propagates: if the other side holds further channels, those may also need cancelling

The paper proves that this system satisfies preservation, progress, deadlock freedom, confluence, and termination. Well-typed programs cannot get stuck, even with exceptions and cancellations.

**Implicit vs explicit cancellation.** Fowler et al. support both. Explicit cancellation (`cancel c`) lets a process deliberately tear down a channel. Implicit cancellation happens when `raise` is invoked -- all channels in scope are automatically cancelled. The typing rules ensure that raising an exception cancels exactly the channels that need cancelling, and that handlers properly account for the channels that might or might not still be alive.

**How this informs pane:** The Fowler model is the theoretical foundation for what pane needs: a way to handle the death of a session participant without the survivor panicking. The practical mapping is:

- `cancel` = process crash (client dies, compositor detects the dead channel)
- `try/otherwise` = crash boundary (compositor wraps each client session in a handler that catches cancellation)
- `raise` = not directly needed (pane components don't deliberately raise protocol exceptions -- they either succeed or crash)

The gap between theory and practice: Fowler's system is implemented in Links (a research language with linear types). Rust's `par` crate has no built-in cancellation mechanism. Pane must build this layer.

### Approach 3: Affine session types with cancellation (Lagaillardie et al.)

Lagaillardie, Neykova, and Yoshida (ECOOP 2022) directly addressed the Rust problem in "Stay Safe Under Panic: Affine Rust Programming with Multiparty Session Types." Their system, MultiCrusty, extends multiparty session types with:

- **Implicit cancellation** triggered automatically when a process panics or terminates
- **Explicit cancellation** via deliberate API calls
- **Cancellation propagation** ensuring all participants learn about the failure

The key guarantee: "communication will not get stuck due to error or abrupt termination." When a participant panics:

1. The panic triggers Drop on the session endpoint
2. The Drop implementation sends a cancellation signal to all connected participants
3. Each participant's next `recv()` returns a cancellation result instead of panicking
4. The cancellation propagates transitively through the session network

This is directly applicable to pane's architecture. The insight is that `Drop` on a session endpoint should NOT panic the counterpart -- it should deliver a typed cancellation signal. The counterpart receives `Err(Cancelled)` instead of panicking, and can clean up gracefully.

### Approach 4: Crash-stop failures in session types (Barwell et al.)

Barwell, Scalas, Yoshida, and Zhou (CONCUR 2022) took a semantic approach: rather than modifying the session type syntax, they parameterize the typing system on a behavioral safety property that accounts for crashes. Their framework covers "the spectrum between fully reliable and fully unreliable sessions, via optional reliability assumptions" -- you can specify which participants must remain operational and which may crash.

The framework enables validation of whether sessions satisfy behavioral properties "even in presence of crashes" through model checking. Type safety and protocol conformance are preserved despite crash-stop failures.

**How this informs pane:** The key insight is that not all participants have equal reliability requirements. In pane:

- The compositor (pane-comp) is the root of the desktop. It MUST NOT crash from client failures. It has the highest reliability requirement.
- Infrastructure servers (pane-route, pane-roster, pane-store) are restarted by the init system. They should tolerate client crashes but may themselves crash and be restarted.
- Client applications are the least reliable. They may crash at any time. The system must handle this gracefully.

This hierarchy of reliability maps to Barwell et al.'s "optional reliability assumptions." The session type system can be parameterized to say: "this participant may crash; the protocol must still make progress for the remaining participants."

### Synthesis: the three error layers

Pane's error handling has three distinct layers, each requiring a different mechanism:

1. **Application errors** (within-protocol): handled by explicit `Result` branches in session types. Both parties are alive; the operation failed; the protocol continues. This is monadic error handling lifted into the session type.

2. **Participant crashes** (meta-protocol): handled by cancellation at session boundaries. One party dies; the other detects the death and cleans up. This is the Fowler/Lagaillardie layer.

3. **Infrastructure recovery** (system-level): handled by supervision. A server crashes; the init system restarts it; clients reconnect. This is the Erlang/OTP layer.

These three layers compose. A client sends a request (layer 1: may get an error result). The server crashes mid-request (layer 2: client detects cancellation, transitions to "disconnected" state). The init system restarts the server (layer 3: client reconnects and retries). The composition is: `Result` inside sessions, `Cancelled` at session boundaries, supervision above both.

---

## 3. Supervision and Recovery Patterns

### Erlang/OTP: the canonical model

Erlang's supervision trees are the most thoroughly proven approach to fault tolerance in message-passing systems. The core philosophy: "let it crash." Don't try to handle every possible error in every function. Instead, structure the system so that crashes are contained and recovery is automatic.

The supervision tree is a hierarchy of processes:

- **Workers** do actual computation
- **Supervisors** monitor workers and restart them when they crash

Restart strategies encode dependency relationships:

- **one_for_one**: children are independent. If one dies, only it is restarted. This is the right strategy when each client session is independent -- a crashed client doesn't affect other clients.
- **rest_for_one**: linear dependency. If one child dies, it and all children started after it are restarted. This models initialization order dependencies.
- **one_for_all**: strong interdependency. If any child dies, all are restarted. This models tightly coupled subsystems.

Each supervisor has a restart intensity: maximum N restarts in T seconds. If exceeded, the supervisor itself terminates and the failure escalates to its parent. This prevents restart storms from consuming resources.

The key empirical result, cited in Adopting Erlang: "131/132 of errors encountered in production tended to be heisenbugs" -- transient, non-deterministic failures that restart effectively addresses. Restarting works because it restores known-good state without requiring the specific bug to be found and fixed.

**State recovery.** The hardest part of "let it crash" is not restarting the process -- it's recovering the state. Erlang addresses this through:

- Static state (config files, environment) that survives restart
- ETS tables (shared memory tables) that outlive individual processes
- External persistence (databases, filesystems)
- Supervisor-managed state (the supervisor holds state that children need, passing it on restart)

The discipline: a process's restart must bring it to "a stable, known state." If initialization requires external resources (database connections, network services), that initialization should be decoupled from process startup. The process starts, enters a degraded mode, and acquires resources asynchronously.

### Akka: actors and supervision

Akka adapts the Erlang model to the JVM actor system:

- **Supervision directives**: resume (keep state, ignore failure), restart (clear state, start fresh), stop (permanent termination)
- **Death watch**: any actor can monitor another's termination, receiving a `Terminated` message. This is distinct from supervision (which is parent-child only).
- **Backoff supervision**: wraps child restart with exponential backoff, preventing rapid restart cycles

The critical distinction Akka makes: **supervision** reacts to failures (parent-child), while **death watch** detects termination (any actor can watch any other). In pane's terms: pane-init supervises servers (restarts them on crash), while pane-roster watches servers (detects when they die and updates the service directory).

### BeOS: what happened when things died

BeOS's BLooper model provided implicit crash isolation through the per-thread architecture. Each BLooper ran its own thread; if a handler crashed, it took down its looper's thread but not other loopers. The app_server allocated a thread per client; if a client's thread died, the app_server could detect the dead thread (via port errors or thread status checks) and clean up the client's resources without affecting other clients.

Pavel Cisler (Be Newsletter Issue 3-33) described the practical pattern for detecting dead partners:

> "Lock() is designed to handle being called on Loopers that have been deleted, so if the window is gone, the lock will fail and we'll just bail."

The `Lock()` call on a dead BLooper returned an error rather than crashing. This is the same pattern as Lagaillardie's cancellation: instead of panicking when the counterpart is gone, you get an error value that you can handle.

George Hoffman (Issue 2-36) described what happened when a window thread became unresponsive:

> "If a window thread becomes unresponsive, and the user continues to provide input... its message queue will fill up. If this happens, the app_server will start to throw away update messages and input intended for the window, and this can cause erratic behavior."

The app_server didn't crash when a client became unresponsive. It degraded: dropping messages rather than blocking. This is a form of circuit breaker -- the server protects itself by shedding load from a failing client.

Haiku's launch_daemon (the modern evolution) added automatic restart for services. When a service process crashed, launch_daemon restarted it. The pre-created port system meant that messages could queue for the restarting service, providing continuity across restarts.

### How supervision maps to pane

Pane's architecture already has a supervision hierarchy, though it's not yet articulated as one:

```
pane-init (init system abstraction)
  |-- pane-comp (compositor)
  |     |-- client session 1 (pane-shell, editor, etc.)
  |     |-- client session 2
  |     '-- client session N
  |-- pane-route (router)
  |     |-- client session 1
  |     '-- client session N
  |-- pane-roster (roster)
  |     |-- registered server 1
  |     '-- registered server N
  '-- pane-store (attribute store)
```

The supervision strategies by level:

- **pane-init supervises infrastructure servers**: restart on crash (one_for_one -- servers are independent). If a server exceeds its restart budget, escalate to the user (notification: "pane-route has crashed repeatedly").
- **pane-comp supervises client sessions**: on client death, clean up the client's panes (remove from layout, free resources). No restart -- clients are launched by the user or by pane-roster, not by the compositor.
- **pane-roster watches everything**: detects server death (updates the service directory), detects client death (updates the app list), facilitates session restore on restart.

---

## 4. Monadic Composition of Recovery Strategies

### Recovery as a monadic operation

A fallible operation wrapped in retry logic is a monad transformer. The base operation returns `Result<T, E>`. The retry wrapper transforms it into a computation that retries on failure:

```rust
// Base operation
fn connect(addr: &str) -> Result<Connection, ConnectError>;

// Retry wrapper (monadic transformer)
fn with_retry<T, E>(
    max_attempts: u32,
    delay: Duration,
    op: impl Fn() -> Result<T, E>,
) -> Result<T, E> {
    for attempt in 0..max_attempts {
        match op() {
            Ok(v) => return Ok(v),
            Err(e) if attempt < max_attempts - 1 => {
                std::thread::sleep(delay * 2u32.pow(attempt));
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}
```

This is a monad transformer: it takes a `Result`-producing computation and wraps it in retry logic, producing another `Result`-producing computation. The caller doesn't know or care that retries happened -- it gets a `Result<T, E>` either way.

### Circuit breaker as a state machine monad

A circuit breaker wraps a fallible operation with state:

- **Closed** (normal operation): requests pass through. If failure rate exceeds threshold, transition to Open.
- **Open** (rejecting): requests immediately fail with a "circuit open" error. After a timeout, transition to Half-Open.
- **Half-Open** (probing): one request passes through. If it succeeds, transition to Closed. If it fails, transition to Open.

The circuit breaker is a state machine that composes with the underlying operation monadically:

```rust
enum CircuitState { Closed, Open(Instant), HalfOpen }

struct CircuitBreaker<E> {
    state: CircuitState,
    failure_count: u32,
    threshold: u32,
    timeout: Duration,
    _phantom: PhantomData<E>,
}

impl<E> CircuitBreaker<E> {
    fn call<T>(&mut self, op: impl FnOnce() -> Result<T, E>) -> Result<T, CircuitError<E>> {
        match self.state {
            CircuitState::Open(since) if since.elapsed() < self.timeout => {
                Err(CircuitError::Open)
            }
            CircuitState::Open(_) => {
                self.state = CircuitState::HalfOpen;
                self.probe(op)
            }
            CircuitState::HalfOpen => self.probe(op),
            CircuitState::Closed => self.attempt(op),
        }
    }
}
```

The circuit breaker composes with session types naturally. When a client reconnects to a restarted server, the reconnection logic can be wrapped in a circuit breaker: if the server keeps crashing on reconnect, stop trying and report the failure to the user rather than hammering the restart loop.

### Fallback chains

Fallback is monadic `or_else` (the dual of `and_then`):

```rust
fn or_else<F>(self, f: F) -> Result<T, E>
where F: FnOnce(E) -> Result<T, E>
```

Where `and_then` chains on success, `or_else` chains on failure. A fallback chain:

```rust
primary_server()
    .or_else(|_| secondary_server())
    .or_else(|_| cached_result())
    .or_else(|_| degraded_mode())
```

Each step is tried only if the previous one failed. The chain stops at the first success. This is the monadic dual of the happy-path pipeline.

For pane, fallback applies to service resolution. When a client needs pane-route:
1. Try the registered address from pane-roster
2. If roster is down, try the well-known socket path
3. If the socket doesn't exist, operate without routing (degraded mode -- tag text isn't actionable but the shell still works)

### Graceful degradation

Degraded operation is a partial result monad. Instead of `Result<T, E>` (all or nothing), use a type that carries partial results:

```rust
enum Degraded<T, W> {
    Full(T),
    Partial(T, Vec<W>),  // result with warnings
    Failed(Vec<W>),       // no result, only warnings
}
```

This composes like `Result` but preserves partial progress. A pane that loses its connection to pane-route can still function -- it just can't route text. A pane that loses connection to pane-store can still display files -- it just can't show attribute columns. The degradation is visible to the user (a status indicator, a dimmed route button) but the pane doesn't die.

### How recovery strategies compose with session types

The question is whether retry/fallback/circuit-breaker can be expressed WITHIN the session type, or whether they must wrap AROUND it.

**Within the session type.** A protocol that includes retry:

```rust
enum ConnectResult {
    Connected(Send<ActiveSession>),
    Retry(Recv<Duration, Recv<ConnectResult>>),  // server says "try again after delay"
    Refused(Send<RefuseReason>),
}
type ConnectSession = Send<ConnectRequest, Recv<ConnectResult>>;
```

The retry is part of the protocol. The server tells the client to wait and retry. The session type ensures both sides agree on the retry protocol. This is clean but limited -- it only handles server-directed retry, not client-side retry after a crash.

**Around the session type.** For crash recovery, the retry wraps the entire session:

```rust
fn with_session_retry<T>(
    max_attempts: u32,
    connect: impl Fn() -> Result<SessionEndpoint, ConnectError>,
    run: impl Fn(SessionEndpoint) -> Result<T, SessionError>,
) -> Result<T, FatalError> {
    for attempt in 0..max_attempts {
        match connect().and_then(|ep| run(ep)) {
            Ok(v) => return Ok(v),
            Err(SessionError::Cancelled) if attempt < max_attempts - 1 => {
                // Server crashed. Wait for restart, try again.
                wait_for_service("pane-route")?;
                continue;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err(FatalError::ServiceUnavailable)
}
```

The session type governs the protocol within a single connection. The retry logic governs reconnection across crashes. The two compose cleanly because the session type is the inner layer and the retry is the outer layer. The session type doesn't need to know about retry. The retry logic doesn't need to know about the session protocol.

This is the right decomposition for pane. Application-level errors are inside the session type (Result branches). Crash recovery is outside the session type (reconnection logic in the kit layer). Supervision is outside both (init system restarts servers).

---

## 5. The Linear Types Gap in Rust

### Affine vs linear: the fundamental problem

Rust's type system is affine: values can be used at most once. Linear types require values to be used exactly once. The difference is whether silent dropping is permitted.

For session types, this matters critically. A session endpoint represents a contractual obligation: "I will complete this protocol." Dropping the endpoint means abandoning the contract. In a linear type system, the compiler rejects the drop -- you MUST complete the protocol (or explicitly cancel it). In Rust's affine system, the compiler allows the drop, and the counterpart discovers the abandonment at runtime (via a panic from `par`'s dropped oneshot channel).

Aria's "The Pain of Linear Types in Rust" identifies the core tension: Rust can't have linear types without solving the panic problem. A `panic!()` unwinds the stack, dropping all values in scope. If those values are linear (must-use), what happens? Two options:

1. **`no_panic` effect.** Functions that hold linear values can only call functions that are statically guaranteed not to panic. This is sound but infectious -- it propagates through the call graph, restricting what code can do while holding a session endpoint.

2. **Destructor bombs.** If a linear value is dropped during unwinding, abort the process. This is sound but brutal -- any panic in a function holding a session endpoint becomes a process abort.

Yoshua Wuyts proposes a third path: "linear Drop" -- a destructor that runs when a linear type is dropped by the runtime (panic, scope exit) but is skipped when the value is manually consumed. This enables error recovery: the linear Drop on a session endpoint can send a cancellation signal, providing the counterpart with a typed notification rather than a panic.

### What par does today

The `par` crate's `Send` and `Recv` types are `#[must_use]` -- the compiler warns if they're dropped. But warnings are not errors. A crashed process doesn't heed warnings; it drops everything.

When a `Send` endpoint is dropped, the oneshot sender inside it is destroyed. When the counterpart calls `recv()`, the oneshot receiver returns `Err(Cancelled)` -- but `par`'s `recv()` implementation converts this to a panic. The panic is the problem.

The `#[must_use]` attribute covers the common case (programmer forgets to use a session endpoint) but not the crash case (process dies, all values are dropped during unwinding or thread teardown).

### Practical solutions for pane

Given that Rust doesn't have linear types and won't for the foreseeable future, pane needs practical workarounds for the affine gap:

**1. Cancellation-aware session wrapper.**

Wrap `par`'s `Recv` in a layer that converts the "sender dropped" panic into an `Err(SessionCancelled)`:

```rust
/// A session receive that returns Err instead of panicking
/// when the counterpart drops their endpoint.
async fn recv_or_cancel<T, S: Session>(
    r: Recv<T, S>
) -> Result<(T, S), SessionCancelled> {
    match std::panic::catch_unwind(AssertUnwindSafe(|| {
        futures::executor::block_on(r.recv())
    })) {
        Ok((value, cont)) => Ok((value, cont)),
        Err(_) => Err(SessionCancelled),
    }
}
```

This is the Lagaillardie approach adapted to `par`: catch the panic at the session boundary and convert it to a `Result`. The compositor never sees the panic -- it sees `Err(SessionCancelled)` and cleans up the dead client's panes.

The limitation of `catch_unwind`: it only catches unwinding panics (`panic = "unwind"` in Cargo.toml). If pane is compiled with `panic = "abort"`, crashes abort the process immediately and `catch_unwind` doesn't help. Pane should use `panic = "unwind"` for the compositor and infrastructure servers.

**2. Session boundary threads.**

Each client session runs in its own thread (the BLooper model). If the session thread panics -- whether from a protocol error, a dropped endpoint, or any other cause -- the panic is contained to that thread. The compositor's main thread (the calloop event loop) detects the thread death via `JoinHandle::join()` returning `Err`:

```rust
// Compositor spawns a thread per client session
let handle = std::thread::spawn(move || {
    run_client_session(session_endpoint, client_state)
});

// Later, check if the session died
match handle.join() {
    Ok(()) => { /* clean exit */ }
    Err(panic_info) => {
        // Client session panicked. Clean up its panes.
        remove_client_panes(client_id);
        log::warn!("client {} session panicked: {:?}", client_id, panic_info);
    }
}
```

This is the sentinel thread pattern. The session thread is a crash boundary: panics inside it don't propagate to the compositor's event loop. The compositor detects the death and cleans up. This maps directly to BeOS's architecture where the app_server had a thread per client and could detect dead clients.

**3. Heartbeat protocol.**

For the transport layer (unix sockets, not par's in-memory channels), detect dead counterparts via heartbeat messages:

```rust
enum ProtocolMessage {
    // Normal protocol messages
    Request(PaneRequest),
    Event(PaneEvent),
    // Heartbeat
    Ping,
    Pong,
}
```

If the compositor doesn't receive a `Pong` within a timeout, the client is presumed dead. The compositor closes the socket and cleans up. This handles the case where a client process is killed by the OS (SIGKILL) without any unwinding or Drop execution.

**4. Custom Drop on session endpoints.**

For the transport bridge (where pane maps par's in-memory sessions to unix socket communication), implement `Drop` on the socket-backed session endpoint to send a cancellation message:

```rust
struct SocketSession {
    socket: UnixStream,
    alive: bool,
}

impl Drop for SocketSession {
    fn drop(&mut self) {
        if self.alive {
            // Best-effort cancellation message
            let _ = self.socket.write_all(&cancel_message());
            let _ = self.socket.shutdown(std::net::Shutdown::Both);
        }
    }
}
```

The `Drop` sends a cancellation message if the session is still alive (hasn't been explicitly closed). This is the "linear Drop" pattern from Wuyts: the destructor handles the implicit drop case, while normal protocol completion skips it by setting `alive = false`.

The "best effort" nature (`let _ = ...`) is important. During panic unwinding, the socket write might fail. That's fine -- the heartbeat/socket-close detection is the backup. The Drop-based cancellation is an optimization for the common case (process exit, not crash).

---

## 6. Architecture for Pane

### The three-layer composition

Pane's error handling architecture is three layers, each compositional:

**Layer 1: Application errors (monadic, within the session type)**

Operations that can fail return `Result` as part of the protocol:

```rust
enum PaneActive {
    WriteCells(Send<CellRegion, Recv<Result<(), WriteError>, Recv<PaneActive>>>),
    SetTag(Send<TagLine, Recv<Result<(), TagError>, Recv<PaneActive>>>),
    // ...
    Close,
}
```

Error handling is monadic: the client receives `Result`, matches on it, decides whether to retry, degrade, or report. The session continues in both cases. The compositor's error response is typed -- the client knows exactly what errors are possible for each operation.

Not every operation needs a `Result`. Fire-and-forget operations (like cell writes during normal rendering) can be unacknowledged for performance. The protocol can distinguish:

```rust
enum PaneActive {
    // Fast path: no acknowledgment, compositor processes asynchronously
    WriteCells(Send<CellRegion, Recv<PaneActive>>),
    // Acknowledged: compositor confirms or reports error
    WriteCellsAcked(Send<CellRegion, Recv<Result<(), WriteError>, Recv<PaneActive>>>),
    // ...
}
```

This mirrors BeOS's distinction between async and sync app_server calls: the fast path skips the round-trip, the careful path gets confirmation.

**Layer 2: Session boundaries (cancellation-aware, wrapping the session)**

Every client session runs in its own thread. The thread is a crash boundary:

```rust
fn spawn_client_session(
    socket: UnixStream,
    compositor_tx: Sender<CompositorEvent>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            run_session(socket)
        }));

        match result {
            Ok(Ok(())) => {
                compositor_tx.send(CompositorEvent::ClientDisconnected {
                    reason: DisconnectReason::Clean,
                }).ok();
            }
            Ok(Err(e)) => {
                compositor_tx.send(CompositorEvent::ClientDisconnected {
                    reason: DisconnectReason::Error(e),
                }).ok();
            }
            Err(panic_info) => {
                compositor_tx.send(CompositorEvent::ClientDisconnected {
                    reason: DisconnectReason::Crash(format!("{:?}", panic_info)),
                }).ok();
            }
        }
    })
}
```

The compositor's event loop receives `ClientDisconnected` events through the same channel it receives normal protocol events. A client crash is an event, not a panic. The compositor handles it the same way it handles any other event: remove the client's panes from the layout, free resources, log the event.

This is the key monadic insight: **component failure is a value, not an exception.** The compositor's event handler:

```rust
match event {
    CompositorEvent::ClientRequest { id, request } => {
        handle_request(id, request);
    }
    CompositorEvent::ClientDisconnected { id, reason } => {
        // Same code path whether clean disconnect, error, or crash
        remove_client_panes(id);
        match reason {
            DisconnectReason::Clean => { /* nothing to log */ }
            DisconnectReason::Error(e) => log::info!("client {id}: {e}"),
            DisconnectReason::Crash(info) => log::warn!("client {id} crashed: {info}"),
        }
    }
}
```

No special error handling path. Disconnect is just another variant in the event enum. The compositor's event loop is total over its event type.

**Layer 3: Supervision (init system, wrapping the process)**

Infrastructure servers are supervised by the init system (via pane-init):

```
# pane-init configuration (conceptual)
[pane-comp]
type = service
restart = always
restart-limit = 5 per 60s  # escalate if exceeded

[pane-route]
type = service
restart = always
restart-limit = 10 per 60s

[pane-roster]
type = service
restart = always
restart-limit = 10 per 60s
```

When a server crashes and restarts, clients detect the restart through socket errors (connection reset) and reconnect. The kit library (pane-app) handles reconnection transparently:

```rust
impl RouteClient {
    fn send(&mut self, msg: RouteMessage) -> Result<RouteResult, RouteError> {
        match self.try_send(&msg) {
            Ok(result) => Ok(result),
            Err(RouteError::Disconnected) => {
                // Server probably restarted. Reconnect and retry once.
                self.reconnect()?;
                self.try_send(&msg)
            }
            Err(e) => Err(e),
        }
    }
}
```

The reconnection logic is in the kit, not in application code. Applications call `route_client.send(msg)` and get a `Result`. They don't know or care that a reconnection happened behind the scenes. This is the BeOS pattern: the kit library hides the client-server communication complexity behind a synchronous-feeling API.

### The roster as liveness oracle

pane-roster tracks who's alive. When a server dies and restarts, it re-registers with roster. When a client dies, the compositor informs roster (or roster detects it via its own session monitoring). Roster's liveness information is available to any component:

```rust
// pane-roster's service directory
type RosterSession = Send<Registration, RosterActive>;
enum RosterActive {
    Query(Send<RosterQuery, Recv<RosterResponse, Recv<RosterActive>>>),
    WatchService(Send<ServiceId, Recv<ServiceStatus, Recv<RosterActive>>>),
    Disconnect,
}

enum ServiceStatus {
    Alive { address: SocketAddr, since: Instant },
    Dead { last_seen: Instant, restart_count: u32 },
    Unknown,
}
```

A client can subscribe to service liveness: "tell me when pane-route comes back." This enables reconnection without polling: the client waits for a roster notification rather than hammering the socket.

### Compositor crash: the nuclear case

If the compositor itself crashes, everything goes. This is equivalent to the X server crashing -- no recovery is possible within the session.

The mitigation is defense in depth:

1. The compositor's event loop MUST NOT panic. All client interactions are wrapped in catch_unwind. All rendering errors are caught and logged. A bad client frame doesn't crash the compositor -- it shows a placeholder.

2. The compositor's code must be minimal. The less code runs in the compositor's critical path, the fewer crash opportunities. Heavy computation (layout algorithms, text shaping) should be pure functions that can't panic on valid input, with input validation at the boundary.

3. Session state persistence. The compositor periodically serializes its layout state (pane positions, tag set visibility, split ratios) to the filesystem. On restart, it restores the layout. Combined with client session restore (each client re-registers its panes), a compositor crash becomes a brief visual interruption rather than total work loss.

4. The init system restarts the compositor. pane-init monitors pane-comp and restarts it on crash. The restart budget should be generous but finite -- if the compositor crashes 5 times in a minute, something is fundamentally wrong and the user needs to be notified.

### Summary: the monadic error handling stack

```
                       Supervision (pane-init)
                      /                       \
                Restart server              Notify user
                      |                    (if budget exceeded)
                      v
               Session Boundary
              /                  \
    catch_unwind              JoinHandle::join()
    on session thread         on compositor thread
              |                       |
              v                       v
    Err(SessionCancelled)     CompositorEvent::ClientDisconnected
              |                       |
              v                       v
         Kit reconnect          Remove panes from layout
              |
              v
      Application code sees:
      Result<T, AppError>
```

Each layer is compositional:

- **Result** composes via `and_then` / `?` -- monadic error propagation
- **Session boundaries** compose via thread spawn/join -- crash containment
- **Supervision** composes via restart strategies -- init system policy

No layer knows about the other layers' internals. Application code uses `Result`. The kit wraps sessions in crash boundaries. The init system wraps processes in restart policies. The composition is by nesting, not by coupling.

This is the monadic insight applied to system architecture: error handling at each level is a functor (maps over the success case) and a monad (chains through the error case). The levels compose by wrapping, with each level's error type being a value in the level above.

---

## How This Informs Pane's Design

The research points to several concrete design decisions:

**1. Session types should include Result branches for application errors.** Not every operation needs acknowledgment (performance matters for cell writes), but operations that can fail meaningfully (create pane, set widget tree) should return typed errors as part of the protocol. This is the monadic layer.

**2. Every client session must run in its own thread with catch_unwind.** This is the BLooper pattern with modern Rust crash containment. The compositor's event loop must never see a panic from client code. Client death is an event, not a crash.

**3. The kit layer must handle reconnection transparently.** When a server restarts, the kit detects the socket error, reconnects, and retries. Application code sees a `Result`. This is the monadic error handling in the kit API -- the retry/reconnect logic is hidden behind `and_then`.

**4. pane-roster tracks liveness and provides watch notifications.** Components subscribe to liveness changes rather than polling. This enables efficient reconnection and allows the UI to reflect service status.

**5. The compositor must serialize its state for crash recovery.** Layout, tag sets, split ratios -- anything needed to reconstruct the visual state after a restart. Combined with client session restore, compositor crashes become survivable.

**6. pane-init defines restart budgets per server.** Restart limits prevent crash loops. Budget exhaustion escalates to user notification. The supervision policy is declarative configuration, not code.

**7. Cancellation-aware session endpoints for the transport bridge.** The layer that maps par's session types onto unix sockets must handle the affine gap: Drop sends a cancellation message, recv catches panics and returns `Err(SessionCancelled)`. This is the practical bridge between Rust's affine types and the linear discipline session types require.

These decisions compose into a system where: application errors are handled by code (Result matching), session crashes are handled by the runtime (thread boundaries + catch_unwind), and server crashes are handled by the infrastructure (init system + roster). No single mechanism handles all error cases. The composition of three mechanisms, each simple and each at the right level, handles the full spectrum.
