---
name: Phase 3 channel topology split decisions
description: Plan 9 perspective on multi-source looper design — mount metaphor, priority deferral, clipboard async channel, no fd table, no premature generalization
type: project
---

Phase 3 channel topology split consultation (2026-03-31).

## Key decisions

1. **Mount metaphor for mental model, not literal API.** Adding a service channel = Plan 9 `mount` into the looper's namespace. Mechanism is `handle.insert_source()` at setup time (static) or command-through-existing-channel (dynamic, same pattern as timer registration). No literal namespace or fd table.

2. **Priority: defer, don't build.** Batch-sorting (not separate dispatch phases) is the right mechanism when needed. Current batch sizes don't warrant it. Compositor-side coalescing is the real fix. Keep events tagged by source so sorting is possible later.

3. **Clipboard: async channel, not synchronous file-like.** Network latency + lock contention + .plan policy checks make synchronous blocking wrong for the kit API. Channel-per-service matches Plan 9 `select()` across multiple mounted fds. Synchronous interface belongs in pane-fs FUSE layer only.

4. **No fd table / generic registry.** calloop IS the multiplexer. `LooperSetup` struct for static topology is sufficient. Each channel callback converts typed events into LooperMessage::Posted(Message::...) — the batch is the unification point.

5. **Build concrete clipboard channel, not generic framework.** One example gives wrong abstractions. Extract patterns after observer (second consumer) exists. Matches Plan 9 philosophy: `mount(2)` was concrete, pattern emerged from repeated use.

6. **Batch integration via option (a).** Clipboard channel callback wraps ClipboardEvent into Message variants, pushes as LooperMessage::Posted. Existing coalesce/filter/dispatch pipeline unchanged. Widen batch type (option b) only when coalescing needs source-level discrimination.

**Why:** Lane asked for Plan 9 perspective on Phase 3 multi-source looper design. Five specific questions answered.

**How to apply:** Reference when implementing clipboard channel registration in looper.rs. Key constraint: LoopHandle is !Send, so dynamic registration must go through command channel to looper thread.
