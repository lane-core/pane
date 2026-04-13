---
type: architecture
status: current
supersedes: []
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [shared-state, Observable, IntraMessenger, AppState, Transport, thread-per-pane, zero-copy, ArcSwap, MonadicLens, snapshot]
related: [decision/thread_per_pane, architecture/proto, architecture/app, architecture/compositor, architecture/fs]
agents: [be-systems-engineer, plan9-systems-engineer, session-type-consultant, optics-theorist]
---

# Architecture: Shared-State Primitives

## Summary

Thread-per-pane enables shared-state primitives that exploit
locality while preserving distribution fallback. The process
is the application and namespace boundary; threads are panes
within that application sharing address space.

## Execution model

```
Application (process) ← namespace boundary, AppState
  ├── pane thread 1   ← shared memory, IntraMessenger
  ├── pane thread 2   ← shared memory, IntraMessenger
  └── pane thread 3   ← shared memory, IntraMessenger
```

- Process = application = namespace boundary
- Threads = panes = shared address space collaborators
- Cross-application = IPC (ServiceHandle, wire protocol)
- The process manages namespace visibility on behalf of
  its threads (predicate-based DeviceRegistry filter)

## The two primitives

### Observable\<T\> — frame-rate shared observation

ArcSwap + generation counter. One pane writes, others
observe at frame rate. Readers never block writers (SH1).
Writes are atomic swap — no partial state visible (SH2).

```rust
pub struct Observable<T> {
    current: arc_swap::ArcSwap<T>,
    generation: AtomicU64,
}
```

- `snapshot()` → `(Arc<T>, u64)`: one atomic load + Arc clone
- `update(f)`: swap + generation increment
- `changed_since(gen)` → `bool`: one atomic load (~1ns)

Observable is NOT an optic. It's a concurrency primitive
that optics compose with. Optics start at the snapshot
boundary — once you have `Arc<T>`, MonadicLens operates
on the owned snapshot normally, with lens laws holding
unconditionally (SH6).

```
Observable<S>  ──snapshot()──→  Arc<S>  ──MonadicLens──→  A
   (shared,                   (owned,                  (projected,
    process-wide)              per-pane)                per-lens)
```

### IntraMessenger\<P\> — zero-copy intra-process messaging

Calloop bounded channel with move semantics. No
serialization, no copy. Falls back to ServiceHandle for
cross-process targets.

```rust
pub struct IntraMessenger<P: Protocol> {
    direct: Option<calloop::channel::Sender<Transport<P::Message>>>,
    wire_fallback: ServiceHandle<P>,
}
```

- Same-process send: ~10-50ns (channel send + atomic wake)
- Cross-process fallback: ~20μs (serialize + syscall)

Transport enum (crate-internal):
```rust
pub(crate) enum Transport<M: Message> {
    Direct(M),    // zero-copy move
    Wire(Vec<u8>), // serialized bytes
}
```

Handler never sees Transport — dispatch unwraps
transparently.

**Backpressure (SH4):** IntraMessenger MUST use bounded
channels with try_send semantics. The wire path's
max_outstanding_requests (D9) cap applies to the direct
path too. Unbounded calloop channels would allow senders
to flood receivers.

**Teardown (SH5):** When a pane thread exits, the
destruction sequence (I9) must explicitly tear down direct
channels and fire fail_connection for all peers using
IntraMessenger. calloop channel Sender::send() doesn't
return transport-EOF-style errors — explicit cleanup needed.

## SharedLens — REMOVED

Originally proposed as MonadicLens over Arc<RwLock<S>>.
Removed after analysis by optics-theorist and session-type-
consultant:

- **PutGet fails under concurrency.** Pane A sets `a`, pane
  B sets `b` between set and view, pane A reads `b`. Law
  violated.
- **GetPut fails similarly.** Concurrent write between view
  and set overwrites the intervening change.
- **RwLock causes I2 violation (SH3).** Write contention
  between looper threads blocks dispatch — exactly what I2
  prohibits.

**Replacement:** Observable + MonadicLens on owned snapshots.
Each pane snapshots the Observable (Arc clone), lenses over
the owned snapshot. Laws hold because the snapshot is
immutable and single-threaded. Mutations go through
Observable::update(), which atomically swaps state.

## AppState — typed be_app

Process-level state registry. Not a global — passed
explicitly to PaneBuilder. Write-once at startup, then
read-only registry lookups.

```rust
pub struct AppState {
    registry: RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}
```

- `register<T>(value)`: write-once at startup
- `get<T>()` → `Option<Arc<T>>`: type-safe lookup

Observable<T> instances stored in AppState ARE mutable, but
mutations go through ArcSwap (not through AppState's
RwLock). AppState's RwLock guards only the registry
structure, which is write-once.

Heritage: BeOS be_app global, but explicit (no global
pointer), type-safe (no dynamic_cast), safe (Arc, not raw
pointer).

## Invariants

| ID | Invariant | Enforcement |
|---|---|---|
| SH1 | Observable reads are non-blocking | Structural — ArcSwap API |
| SH2 | Observable writes are atomic swap | Structural — ArcSwap::store |
| SH3 | No cross-pane lock contention on looper threads | Structural — no RwLock in shared state |
| SH4 | IntraMessenger backpressure parity with wire path | Runtime — bounded channel, try_send, D9 cap |
| SH5 | Direct channel teardown fires fail_connection | Runtime — destruction sequence extended |
| SH6 | Lens laws hold on snapshots, not shared state | Structural — MonadicLens on owned Arc<S> |

## Application patterns

### Multi-view editing

```
Observable<Document> in AppState
  ├── code pane:    snapshot → MonadicLens<Doc, Source>
  ├── preview pane: snapshot → MonadicLens<Doc, Html>
  └── outline pane: snapshot → MonadicLens<Doc, Outline>
```

Edit in code pane → Observable::update() → generation
increments → preview/outline check changed_since() on
next pulse (~1ns) → re-snapshot and re-render. Zero copy
between panes.

### Live preview

Preview checks `changed_since()` on pulse (~1ns). If
changed, snapshots (~5ns), lenses to Html, re-renders.
No IPC latency.

### Instrumentation

```rust
struct PaneMetrics {
    frames_rendered: AtomicU64,
    dispatch_latency_ns: AtomicU64,
}
```

Profiler pane reads atomics at frame rate — zero overhead
on observed panes, zero serialization.

## Performance tiers

```
Same process:  Observable + MonadicLens / IntraMessenger  (~1-50ns)
                            ↓ (fallback)
Cross process: pane-fs snapshot / ServiceHandle           (~20μs)
```

The process boundary is contingent, not architectural.
host_as_contingent_server applied to the process boundary.

## Crate impact

| Crate | Change |
|---|---|
| pane-proto | Internal Transport enum; no SharedLens |
| pane-session | No change (wire stays; intra-process bypasses it) |
| pane-app | Looper accepts bounded Transport channel; Messenger gains intra_messenger(); PaneBuilder gains with_app_state(); destruction sequence extended (SH5) |
| pane-kernel | DeviceRegistry shared via Arc within process |
| pane-compositor | Buffer sharing optimization (optional) |
| pane-fs | Observable replaces clone-based snapshots; generation counter enables lazy re-snapshot |

## Provenance

Designed 2026-04-12. Three consultation rounds:
1. Be + Plan 9 agents recommended process-per-pane
2. Lane pushed back; Be + Plan 9 identified shared-memory
   capabilities (Observable, SharedLens, IntraMessenger)
3. Session-type + optics agents validated: SharedLens
   removed (lens laws fail under concurrency, I2 violation
   via RwLock). Observable + MonadicLens on snapshots is
   the correct design. SH1-SH6 invariants established.

Lane clarified the execution model: process = application =
namespace boundary, threads = panes within the application.
The process manages namespace visibility on behalf of its
threads.
