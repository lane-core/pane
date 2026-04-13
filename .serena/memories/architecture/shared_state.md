---
type: architecture
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [shared-state, Observable, SharedLens, IntraMessenger, AppState, Transport, thread-per-pane, zero-copy, ArcSwap]
related: [decision/thread_per_pane, architecture/proto, architecture/app, architecture/compositor, architecture/fs]
agents: [be-systems-engineer, plan9-systems-engineer, pane-architect]
---

# Architecture: Shared-State Primitives

## Summary

Thread-per-pane enables three shared-state primitives that
exploit locality while preserving distribution fallback.
These compose with pane's existing optics and messaging
layers.

## The three primitives

### Observable\<T\> — frame-rate shared observation

ArcSwap + generation counter. One pane writes, others
observe at frame rate. Readers never block writers.

```rust
pub struct Observable<T> {
    current: arc_swap::ArcSwap<T>,
    generation: AtomicU64,
}
```

- `snapshot()` → `(Arc<T>, u64)`: one atomic load + Arc clone
- `update(f)`: swap + generation increment
- `changed_since(gen)` → `bool`: one atomic load (~1ns)

Use case: document models, settings, any state read at
frame rate but written occasionally. Replaces pane-fs's
clone-based snapshot model for intra-process state.

### SharedLens\<S, A\> — shared optic projections

MonadicLens over `Arc<RwLock<S>>`. Two panes viewing the
same state through different projections. Lens laws hold
(the lock is orthogonal).

```rust
pub struct SharedLens<S, A> {
    state: Arc<RwLock<S>>,
    inner: MonadicLens<S, A>,
}
```

- `view()` → A: takes read lock, applies lens view
- `set(value)` → Vec<Effect>: takes write lock, applies
  lens set, returns effects for coordination

Use case: multi-view editing (code view, preview, outline
all projecting the same document through different lenses).

### IntraMessenger\<P\> — zero-copy intra-process messaging

Calloop channel with move semantics. No serialization, no
copy. Falls back to ServiceHandle for cross-process targets.

```rust
pub struct IntraMessenger<P: Protocol> {
    direct: Option<calloop::channel::Sender<Transport<P::Message>>>,
    wire_fallback: ServiceHandle<P>,
}
```

- Same-process send: ~10-50ns (channel send + atomic wake)
- Cross-process fallback: ~20μs (serialize + syscall)
- 100-1000x faster than wire path
- 10-100x faster than BeOS BDirectMessageTarget (which
  deep-copied)

Transport enum (crate-internal):
```rust
pub(crate) enum Transport<M: Message> {
    Direct(M),    // zero-copy move
    Wire(Vec<u8>), // serialized bytes
}
```

Handler never sees Transport — dispatch unwraps transparently.

## AppState — typed be_app

Process-level state registry. Not a global — passed
explicitly to PaneBuilder.

```rust
pub struct AppState {
    registry: RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}
```

- `register<T>(value)`: write-once at startup
- `get<T>()` → `Option<Arc<T>>`: type-safe lookup

Heritage: BeOS be_app global, but explicit (no global
pointer), type-safe (no dynamic_cast), safe (RwLock on
registry, per-value concurrency via Observable/SharedLens).

## Application patterns

### Multi-view editing

```
Observable<Document> shared via AppState
  ├── code pane:    SharedLens<Document, Source>
  ├── preview pane: SharedLens<Document, Html>
  └── outline pane: SharedLens<Document, Outline>
```

Edit in any view → Observable generation increments →
other views re-render on next frame tick. Zero copy.

### Live preview

Preview pane checks `changed_since()` on pulse (~1ns).
If changed, reads snapshot (Arc clone, ~5ns), re-renders.
No IPC latency — preview sees edit on the very next frame.

### Instrumentation

```rust
struct PaneMetrics {
    frames_rendered: AtomicU64,
    dispatch_latency_ns: AtomicU64,
    messages_received: AtomicU64,
}
```

Profiler pane reads atomics at frame rate — zero overhead
on observed panes, zero serialization.

### Replicants

Live projection of another pane's state via
`Arc<Observable<S>>` + MonadicLens. Not a copy — a view.
Email client → deskbar unread count: one atomic check per
pulse, zero-copy snapshot through a lens.

## Crate impact

| Crate | Change |
|---|---|
| pane-proto | Add SharedLens, internal Transport enum |
| pane-session | No change (wire protocol stays; intra-process bypasses it) |
| pane-app | Looper accepts Transport channel; Messenger gains intra_messenger(); PaneBuilder gains with_app_state() |
| pane-kernel | DeviceRegistry shared via Arc for same-process panes |
| pane-compositor | Buffer sharing optimization (optional) |
| pane-fs | Observable replaces clone-based snapshots |

## Performance tiers

```
Same process:  SharedLens / Observable / IntraMessenger  (~1-50ns)
                            ↓ (fallback)
Cross process: MonadicLens / pane-fs snapshot / ServiceHandle  (~20μs)
```

## Provenance

Designed 2026-04-12. Lane leaned toward thread-per-pane
after steelman analysis showed shared memory enables
fundamentally better multi-view patterns. Be agent found
BeOS apps mostly used shared state for settings/caches
(C++ made shared document models too dangerous). Rust's
type system makes the patterns Be developers wanted but
couldn't safely build natural and correct.
