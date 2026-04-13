---
type: analysis
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [thread-per-pane, shared-memory, namespace, distribution, working-set, hot-reload, zero-copy, two-speed]
related: [agent/plan9-systems-engineer/thread_vs_process_consultation, architecture/kernel, architecture/compositor, architecture/app, decision/host_as_contingent_server]
agents: [plan9-systems-engineer]
sources: [Plan 9 foundational paper (Pike et al 1995), rio/wind.c rfork model, Plan 9 devtab[] kernel-internal dev interface]
---

# Thread-per-pane: What Shared Memory Unlocks

Consultation for Lane on what thread-per-pane enables beyond "it's faster." Five sections.

## 1. Namespace coexistence

Shared memory does NOT undermine the namespace model because pane already separates naming (namespace/pane-fs) from access (typed API / file protocol). Shared memory is a third access path — same naming, faster backend. ServiceHandle<P> already uses closure-erased backends (Box<dyn FnOnce> in ReplyPort). The backend can resolve to Arc<T> for intra-process or wire for cross-process, selected at construction time.

Key requirement: the predicate-based device view (HashSet on DeviceRegistry) must also gate shared-memory access. If pane A can read pane B's Arc<T>, that's a namespace visibility question.

Plan 9 parallel: kernel-internal #devices used function-call dev interface (no 9P serialization), while user-level file servers paid full 9P cost. Same pattern, now available to application code.

## 2. Plan 9 concepts enhanced by shared memory

- **Plumber routing:** MessageFilter<M> matches on typed enum directly, no serialization. Enables keystroke-granularity routing (Plan 9 plumber was user-action-granularity due to 9P overhead).
- **Namespace observation:** MonadicLens view() projects from ArcSwap snapshot at ~10ns. Replaces Plan 9's blocking-read-per-observation-thread model.
- **Device buffer sharing:** Compositor shares frame buffers via Arc<Buffer>. No mmap/fd-passing ceremony.
- **Mount table as concurrent data structure:** DeviceRegistry as DashMap — device additions instantly visible to all panes whose predicate admits them.

## 3. Two-speed architecture

Transparent at ServiceHandle<P> level (same API, different backend). Expose distinction via Address topology metadata (IntraProcess / LocalIpc / RemoteTcp) for monitoring and scheduling, not for dispatch branching.

Key lesson from Plan 9: libdraw diverged from /dev/draw file protocol because two APIs for the same thing creates gravitational pull toward one becoming "real." One ServiceHandle<P> type with multiple backends avoids this.

## 4. Distribution benefits

Clear local/remote boundary improves distribution. The "working set" (panes in one process) is fast and reliable. The distribution boundary (where IPC kicks in) is explicit. This is more honest than Plan 9's location transparency, which worked at Bell Labs but hides failure modes on real networks.

Plan 9 kernel already had this two-speed model internally (#devices via dev interface vs user servers via 9P). Thread-per-pane makes it available to application code.

## 5. Capabilities impossible/impractical with IPC

| Capability | Mechanism | IPC possible? |
|---|---|---|
| Hot-reload handler preserving shared state | Drop handler, load new, hand same Arc refs | Impossible (process restart loses state) |
| Zero-perturbation profiling | Arc<[AtomicU64]> shared counters | Impractical (IPC round-trip perturbs timing) |
| Lock-free event broadcast | SPMC ring buffer, atomics | Impractical at scale (IPC cost linear in pane count) |
| Compositor buffer sharing | Arc<Buffer> pointer swap | Possible but complex (shmget/mmap/fd-passing) |
| Keystroke-granularity routing | Typed pattern match, no serialization | Possible but CPU-expensive |
| Real-time shared document model | Arc<ArcSwap<Rope>>, 10ns snapshot | Possible but requires delta serialization |

Headline: composability at interaction speed. Plan 9 made composition cheap enough for scripting (microseconds). Thread-per-pane makes it cheap enough for every keystroke and frame (nanoseconds).
