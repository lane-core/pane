---
name: C1 looper evolution analysis
description: EAct C1 multi-source select analysis — calloop migration, deadlock invariants, Handler trait split, six soundness conditions
type: project
---

C1 looper evolution from single mpsc to calloop multi-source select analyzed 2026-03-31.

**Verdict**: Conditionally sound. calloop correctly implements EAct sigma (handler store) via defunctionalization — persistent callbacks + shared &mut state vs. EAct's consumed-and-replaced functional handlers.

**Key findings**:
- Per-channel message types (not monolithic Message enum) are required for the type safety improvement to be real
- Bounded-channel deadlock: service->pane notification on full channel creates DLfActRiS connectivity graph cycle. Invariant: services must use try_send (non-blocking) for notifications
- Handler trait: option (b) sub-traits (ClipboardHandler, ObserverHandler, etc.) is correct for C5 MessageInterest
- Timer migration: Messenger can't hold LoopHandle (not Send), needs command channel
- Migration: 3 phases (calloop backend swap, timer migration, channel topology split), each independently testable

**Six invariants for soundness**:
1. Session state partitioning in &mut LooperState
2. Non-blocking service notifications (try_send)
3. Channel-specific message enums
4. Timer registration via command channel
5. Batching preserves drain-and-coalesce semantics
6. IS_LOOPER thread-local extends to all calloop dispatch callbacks

**Why:** Design must be right before implementation — calloop touches the core event loop.
**How to apply:** Reference these invariants during implementation. Full analysis at plan file.
