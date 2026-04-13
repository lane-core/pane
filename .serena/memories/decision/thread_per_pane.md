---
type: decision
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [thread-per-pane, process, shared-memory, Observable, SharedLens, IntraMessenger, AppState, isolation, be_app]
related: [architecture/kernel, architecture/compositor, architecture/app, architecture/proto, decision/host_as_contingent_server]
agents: [plan9-systems-engineer, be-systems-engineer]
---

# Thread-per-pane as default execution model

## Decision

Thread-per-pane is the default. An application is a process;
its panes are threads sharing address space. Process
isolation is opt-in for untrusted/sandboxed applications.

The process is the natural namespace boundary. Threads
share their process's namespace — the process manages
visibility on behalf of its panes (predicate-based
DeviceRegistry filter). This is BeOS's model (BApplication
= process, BWindow = thread) with Plan 9's per-process
namespaces.

## Rationale

Shared address space enables capabilities IPC cannot match:

- Zero-copy data sharing (Arc<T>): ~5ns vs serialize+IPC
- Sub-microsecond coordination (atomics): ~10ns vs ~20μs
- Shared document models (Arc<RwLock<Rope>>)
- Frame-rate observation (atomic generation counter): ~1ns
- Replicant-style live projections of shared state

Be's insight: windows in the same application are
collaborators, not strangers. Treating them as strangers
(serialize, IPC, deserialize) throws away the ability to
build tightly integrated multi-view applications cheaply.

Rust makes this safe. BeOS developers feared shared memory
because C++ had no data-race protection. Rust's type system
(Arc, RwLock, Send/Sync bounds) turns shared-memory
programming from dangerous to natural.

## Design principle

**Locality-transparent messaging with locality-exploiting
data sharing.** Messages look the same regardless of target
location (IntraMessenger falls back to ServiceHandle). Data
sharing exploits locality when available — Arc, Observable,
SharedLens only work in-process, and the patterns that need
them are inherently same-process.

The process boundary is where the fallback engages, not
where the architecture changes. `host_as_contingent_server`
applied to the process boundary itself.

## Evidence

Be agent read Haiku's BMessage dispatch (Message.cpp:
2130-2280). Even Be's "fast path" deep-copied BMessages.
pane's IntraMessenger uses move semantics — zero-copy, 
10-100x faster than Be's fast path.

Real Haiku apps (ShowImage, Terminal, Tracker) used shared
address space primarily for settings/caches (write-once,
read-often) via be_app + BLocker. None did multi-view
shared document models — C++ made it too dangerous.
pane can do what Be developers wanted but couldn't safely.

Plan 9 agent confirmed: the namespace model is not
undermined by shared memory, as long as the two-tier pattern
holds (every operation has a file-protocol path). What would
violate Plan 9's spirit is bypassing the namespace entirely.

## Process isolation opt-in

Available when needed via Linux user namespaces +
mount namespaces + Landlock:
- Untrusted plugins/extensions
- Network-facing services
- User-configured security policy (pane-router decides)
- Compositor enforces at spawn time

## Origin

Lane leaned toward thread-per-pane after the honest
steelman showed shared memory enables fundamentally better
multi-view editing, live preview, instrumentation, and
replicant patterns. Two-round consultation (2026-04-12):
first round recommended process-per-pane; Lane pushed back
asking what thread-per-pane unlocks; second round identified
the three shared-state primitives and the
locality-transparent design principle.
