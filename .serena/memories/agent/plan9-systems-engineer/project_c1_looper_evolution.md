---
name: C1 looper evolution design consultation
description: Multi-source select via calloop replacing single mpsc — Plan 9 perspective on ordering, wire protocol, fd-per-concern model, timer migration
type: project
---

C1 looper evolution: pane-app moves from single mpsc recv_timeout to calloop multi-source select. Each protocol relationship (compositor, clipboard, observer, inter-pane) gets its own typed channel. Consulted 2026-03-31.

## Phase 1 decisions (completed 2026-03-31)
1. **Wire stays single-stream.** Do not split TCP into multiple connections per protocol relationship. Single stream, local demux via dispatcher into per-concern calloop channels. Matches Plan 9 kernel 9P mux pattern.
2. **Source priorities.** Compositor events > inter-pane requests > clipboard/observer notifications. Matches acme's prioritization of keyboard/mouse over filesystem events.
3. **Coalescing contract changes.** Current drain_and_coalesce works on single mpsc. With calloop, coalescing becomes "all events ready when poll returned" — better-defined but needs reimplementation as layer between calloop dispatch and handler delivery.
4. **Backpressure.** Current sync_channel(256) provides bounds. calloop channels may be unbounded — must verify or implement bounded variant.
5. **calloop unifies event loop model** across pane-headless and pane-app. One mental model for debugging distributed issues.

## Phase 2 consultation: timer migration to calloop sources (2026-03-31)
Recommendations provided:

1. **Timers are per-looper resources**, not system-level. Matches Plan 9 alarm(2) being process-scoped. No cross-looper timer aliasing, lifetime bounded by looper.
2. **Cross-thread registration via channel command pattern.** LoopHandle is !Send (Rc<LoopInner>). Workers send timer commands through calloop channel; looper calls handle.insert_source(Timer::...) on its thread. Replaces Timers struct with calloop Timer sources + HashMap<u64, RegistrationToken>.
3. **Cancellation: TimerToken holds LooperSender clone.** cancel() sends Cancel command through channel; looper calls handle.remove(reg_token). Replaces Arc<AtomicBool> polling with deterministic removal from TimerWheel.
4. **Timer callbacks push into LooperState.batch**, not separate Vec<Message>. Timer events participate in coalescing, same dispatch path as channel events. Drop timer-first priority (was artifact of hand-rolled approach).
5. **One-shot and periodic use same Timer source type**, differing only in TimeoutAction return (Drop vs ToDuration).
6. **pane-fs: timers are read-only observable**, not externally controllable. /pane/<id>/timers file for monitoring. ctl-based cancellation deferred.
7. **Distributed: timers run local to looper, wherever looper is.** No distributed clock sync. Timer creation from remote Messenger works via command channel over wire. Timer events cross network as regular events with visible latency.

**Why:** Phase 1 completed (calloop EventLoop replacing mpsc recv_timeout). Phase 2 eliminates hand-rolled Timers struct, completes calloop integration.

**How to apply:** Reference when implementing Phase 2 in pane-app/src/looper.rs. Key risk: TimerToken API change (now needs LooperSender clone for cancel()).
