---
type: reference
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [rumpsteak, smol, tokio, async_runtime, calloop, MPST, channel_model, async_subtyping, message_reordering, pane_translation]
sources: [rumpsteak paper (Cutner/Yoshida/Vassor PPoPP 2022), par 0.3.10 digest (dependency/par), smol-rs/async-channel docs, pane status 2026-04-12]
verified_against: [rumpsteak paper full read all sections including arxiv, par 0.3.10 API digest]
related: [dependency/par, decision/pane_session_mpst_foundation, reference/papers/dlfactris, agent/session-type-consultant/eact_mpst_pane_session_analysis]
agents: [session-type-consultant]
---

# Rumpsteak Runtime Model -> smol Translation for pane

## 1. What Rumpsteak Actually Needs from Tokio

Rumpsteak's Tokio dependency is **thin and mechanical**. From the
paper and its implementation:

### 1a. Channel primitive: futures Sink/Stream (NOT Tokio channels)

Rumpsteak's channel model is **runtime-agnostic by design.** The
Role struct stores `Channel` fields, and the paper states:
"Developers can in fact use any custom channel that implements
Rust's standard `Sink` or `Stream` interfaces [Futures] for
asynchronous sends and receives respectively" (S2 Overview,
Roles paragraph).

The `Sink` and `Stream` traits are from the `futures` crate
(futures::Sink, futures::Stream), NOT from Tokio. This is the
critical finding: **Rumpsteak's session type primitives
(Send/Receive/Select/Branch) are parameterized over any
Sink+Stream, not hardcoded to Tokio channels.**

Internally, Rumpsteak sends a Label enum over reusable channels
(one channel per role-pair per direction). The channel is
reused across session steps — NOT one oneshot per message like
par. This is why Rumpsteak is faster (avoids per-interaction
channel allocation).

### 1b. Task spawning: tokio::spawn for benchmarks only

The paper uses "a multi-threaded asynchronous runtime from
version 1.11.0 of the Tokio library" for benchmarks (S4
Evaluation). The `try_session` function takes an async closure
and drives it — any async executor can drive this.

### 1c. Case study: Tokio ecosystem integration

The HTTP cache case study (S5) uses "Tokio for spawning
concurrent asynchronous tasks, Hyper for interfacing with HTTP
and Fred for communicating with Redis." This is application-
level usage, not framework-level. Rumpsteak itself doesn't
call Tokio APIs in its core.

### 1d. No Tokio-specific features used

No tokio::select!, no tokio::time, no tokio::net, no
tokio::sync in the framework itself. The framework uses:
- futures::Sink trait (for sending)
- futures::Stream trait (for receiving)
- Standard Rust async/await
- Procedural macros for code generation

### Summary: Tokio dependency classification

| Dependency | Where | Replaceable? |
|---|---|---|
| futures::Sink | Core API | NO (but already runtime-agnostic) |
| futures::Stream | Core API | NO (already runtime-agnostic) |
| tokio::spawn | Benchmarks/examples | YES, any executor |
| tokio (runtime) | Benchmark harness | YES, any executor |
| Hyper/Fred/Tokio | HTTP cache case study | Application-level |

## 2. Rumpsteak's Message Reordering Model

### What AMR is

Asynchronous Message Reordering (AMR) is the paper's main
contribution. It allows a role's FSM to be "optimized" by
reordering sends and receives, provided the reordered FSM is
an asynchronous subtype of the original projection.

Two reordering rules (S3 Theory, Definition 3):
- R1: anticipate an input from p before finite inputs NOT from p
- R2: anticipate an output to p before finite inputs (any) and
  other outputs NOT to p

The key insight: **reordering works because channels are
asynchronous (buffered).** A send can be moved earlier because
the message queues in the channel. The receiver doesn't need
to be ready.

### Soundness: Theorem 3.3 (Soundness)

The subtyping algorithm is sound against Ghilezan et al. 2021's
precise async multiparty subtyping relation. If the algorithm
accepts M' <= M, then M' can safely replace M without deadlocks.

### Relevance to pane's token correlation

Pane's out-of-order replies (token-correlated) are NOT the same
as Rumpsteak's AMR. Key distinction:

- **Rumpsteak AMR**: the IMPLEMENTATION reorders sends/receives
  relative to the PROTOCOL SPECIFICATION. The protocol itself is
  ordered; the optimization permutes the implementation.
- **Pane token correlation**: multiple requests are in-flight
  simultaneously, replies come back in arbitrary order. This is
  a PROTOCOL-LEVEL feature (the global type permits it), not an
  implementation optimization.

Pane's request correlation is better modeled as **interleaved
binary sessions** (each request/reply pair is an independent
session, multiplexed over one transport) than as AMR of a single
sequential protocol. This aligns with the analysis in
`decision/pane_session_mpst_foundation` which models each
request/reply as an independent sub-interaction within a `rec Loop`.

However, if pane wanted to add **double-buffering style
optimizations** (e.g., pre-sending credits, pipelining handshake
steps, pre-fetching service declarations), Rumpsteak's AMR
verification would be directly applicable.

## 3. Rumpsteak's Channel Model

### One persistent channel per role-pair per direction

This is the critical architectural difference from par.

Rumpsteak uses **reusable channels** — one Sink/Stream pair per
(role_a, role_b) direction. The Label enum is sent over this
channel repeatedly. Session type state transitions happen in the
TYPE SYSTEM; the underlying channel is reused.

```rust
#[derive(Role)]
struct K(
  #[route(S)] Channel,  // one channel to/from S
  #[route(T)] Channel   // one channel to/from T
);
```

"Internally, rumpsteak sends a Label enum over reusable channels
to communicate with other participants" (S2 Overview, Labels).

### Contrast with par

par uses **one oneshot per message**. Each Send/Recv creates a
new futures::channel::oneshot. The continuation session carries
a new oneshot. This is why par is slower for high-throughput
scenarios (each interaction allocates a channel).

### What this means for pane

Rumpsteak's channel model maps naturally to pane's architecture:
- pane already has one persistent connection per role-pair
  (UnixStream per client-server connection)
- pane already multiplexes message types over that connection
  (discriminant-tagged frames)
- Rumpsteak's Label enum is analogous to pane's ServiceFrame
  discriminants

The translation path is: pane's FrameReader/FrameWriter +
service discriminant routing IS the channel that Rumpsteak's
Sink/Stream would wrap.

## 4. Forwarder/Router Role

### Rumpsteak has no explicit forwarder concept

Rumpsteak models all roles as direct participants in the global
type. The paper's examples (double buffering, FFT, HTTP cache)
define intermediary roles (kernel, proxy) as full participants
with their own FSMs. The "kernel" in double buffering explicitly
receives from source and sends to sink — it is a participant,
not a transparent forwarder.

### Contrast with pane's Server role

Pane's ProtocolServer is a forwarder per [CMS] S5.1 — it routes
messages between Consumer and Provider without inspecting payload.
In Rumpsteak terms, the Server would need its own projected FSM
that receives-and-forwards every message type.

This is not a problem but it does mean: if pane adopts Rumpsteak's
model, the Server role's FSM would be explicit (receive from
Consumer, send to Provider; receive from Provider, send to
Consumer). The cut-elimination optimization from [CMS] that proves
forwarder chains compose would need to be applied at the
verification level, not the runtime level.

## 5. Translation Feasibility: pane as Rumpsteak Runtime

### What maps directly

| Rumpsteak concept | pane equivalent | Status |
|---|---|---|
| Sink/Stream channel | FrameWriter/FrameReader | Exists |
| Label enum | ServiceFrame discriminants | Exists |
| Role struct | Connection/PeerScope | Exists |
| try_session | Handler dispatch context | Exists |
| Async executor | calloop Looper | Exists (sync) |
| Channel reuse | Persistent UnixStream | Exists |

### What needs work

1. **Sink/Stream trait impl for pane's frame IO.** FrameWriter
   needs `impl Sink<Label>` and FrameReader needs
   `impl Stream<Item = Label>`. This is mechanical — wrap the
   existing non-blocking read/write in Sink/Stream adapters.
   BUT: pane's FrameReader/FrameWriter are synchronous (non-
   blocking poll, not async). They need async wrappers.

2. **Async executor integration.** Rumpsteak processes are
   `async fn`. pane's Looper is synchronous (calloop poll).
   Two approaches:
   a) Run Rumpsteak async closures on a minimal executor within
      the Looper's dispatch (block_on each handler invocation)
   b) Make the Looper itself an async executor

   Option (a) is simpler but defeats the purpose of async (the
   Looper thread blocks on each handler). Option (b) requires
   calloop-to-async integration.

3. **Session type API generation.** Rumpsteak generates API types
   from Scribble global types via nuScr. pane would need either:
   a) Write Scribble descriptions, use nuScr + Rumpsteak codegen
   b) Hand-write the session types (Rumpsteak's hybrid approach)
   c) Build pane-specific codegen from the global type in
      `decision/pane_session_mpst_foundation`

### The async gap is the real obstacle

Rumpsteak's core assumption is async/await execution. Each
process is an `async fn` that `.await`s on receive and send
operations. The session type API's `send()` and `receive()`
return futures.

pane's Looper is fundamentally synchronous:
- calloop polls file descriptors
- Callbacks fire synchronously
- Handler::receive is a sync fn(&mut self, msg)
- No futures, no .await in the dispatch path

This is not a "swap Tokio for smol" problem. It's a "pane doesn't
use async at all" problem. The question is whether pane SHOULD
use async, not which async runtime to use.

## 6. smol Architecture (from knowledge + docs)

### Structure

smol is a ~1000-line async runtime composed of three crates:
- **async-executor** — the task executor (spawn, tick, run)
- **async-io** — the I/O reactor (wraps epoll/kqueue via polling)
- **blocking** — thread pool for blocking operations

Plus supporting crates from smol-rs:
- **async-channel** — mpmc channels (bounded + unbounded)
- **futures-lite** — lightweight futures utilities
- **async-lock** — async Mutex, RwLock, Semaphore

### Can async-executor be used standalone?

YES. async-executor is independent of async-io. You can spawn
tasks and poll them manually without any I/O reactor. This is
relevant because pane already has calloop as its I/O reactor.

### Can calloop serve as the I/O reactor?

Partially. calloop already does what async-io does (poll fd
readiness, dispatch callbacks). But calloop and async-io have
different APIs:
- async-io produces futures (Async<TcpStream>)
- calloop produces callbacks (EventSource)

To bridge: calloop could wake async-executor tasks when fds
become ready, but this requires custom glue.

### Minimum viable executor for pane

If pane wanted async at all, the minimum would be:
1. async-executor (or a single-threaded LocalExecutor)
2. A calloop EventSource that ticks the executor
3. Sink/Stream wrappers around FrameReader/FrameWriter that
   use calloop fd readiness to wake futures

This is "calloop drives async-executor" — the Looper's poll
loop includes an executor tick step alongside its existing
EventSource dispatches.

## 7. Verdict

### Rumpsteak's Tokio dependency: trivially replaceable

Rumpsteak is runtime-agnostic at the framework level. Its
Sink/Stream abstraction works with any async runtime. Swapping
Tokio for smol in Rumpsteak's benchmarks/examples would be
mechanical (replace tokio::spawn with smol::spawn, replace
#[tokio::main] with smol::block_on).

### pane-as-Rumpsteak-runtime: conditionally feasible

The channel model maps. The role model maps. The message
reordering model is relevant but not directly needed yet.
The obstacle is async: pane's Looper is synchronous.

Three paths forward:
A) **Don't adopt Rumpsteak's runtime.** Use Rumpsteak's
   VERIFICATION tools (subtyping algorithm, kMC) to validate
   pane's protocol designs, but keep pane's sync Looper. Write
   pane's session types by hand following Rumpsteak's type
   patterns. This preserves pane's architecture.
B) **Hybrid async.** Add a LocalExecutor to the Looper that
   ticks during calloop's poll cycle. Session-typed handlers
   are async fns driven by this executor. calloop remains the
   I/O reactor. This is the "smol inside calloop" approach.
C) **Full async migration.** Replace calloop with smol/async-io.
   This is architecturally invasive and probably not warranted.

**Path A is recommended** for the current phase. Rumpsteak's
main value to pane is its subtyping algorithm and verification
methodology, not its async runtime. pane's sync Looper with
non-blocking I/O is performant (900K msg/sec) and well-tested.
The async gap is a design choice, not a deficiency.

### What Rumpsteak's AMR verification offers pane

If pane ever optimizes its protocol (pipelining handshake steps,
pre-sending service declarations, double-buffering notification
streams), Rumpsteak's subtyping algorithm can verify these
optimizations are deadlock-free. The tool is external to the
runtime — it takes FSMs in, says yes/no. Pane can use it without
adopting Rumpsteak's runtime.
