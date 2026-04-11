---
name: Session type build-vs-buy assessment
description: Assessment of par vs dialectic vs custom session types for pane (2026-03-20) — recommends custom minimal typestate implementation
type: project
---

Assessed par (v0.3.10), dialectic (Bolt Labs), and custom implementation for pane's session type needs. Full analysis at openspec/changes/spec-tightening/research-custom-session-types.md.

**Recommendation: custom minimal typestate implementation.** Three load-bearing reasons:

1. **Transport bridge:** par is in-memory only (oneshot continuation passing can't serialize across sockets). dialectic solves transport but requires async runtime. Custom typestate `Chan<S, Transport>` with postcard over unix sockets is the direct solution.

2. **Crash handling:** par panics on drop (`expect("receiver dropped")`). Haiku's app_server used `set_port_owner()` to make client death visible as error return from `GetNextMessage()`, not crash. Custom implementation returns `Result<NextState, SessionError>` — error is an event, not a panic.

3. **calloop fit:** par's `fork_sync` blocks; dialectic is async-everywhere. Custom implementation registers socket fd with calloop as EventSource — callback-driven, no async bridge needed.

**Scope for Phase 2:** Chan + 5 primitives (Send/Recv/Choose/Offer/End) + UnixSocketTransport + SessionError + calloop EventSource impl. ~500-1000 lines. No generality.

**Formal verification:** Designer should verify primitives in Lean/Agda in parallel (2-3 weeks). Verify compositions incrementally. Don't verify Rust-to-model correspondence until needed.

**Phase 2 acceptance criterion:** pane-comp calloop main thread talks to pane-shell client over unix socket, session-typed, with crash recovery by killing client mid-session.

**Key Haiku reference:** ServerApp.cpp lines 129-134 — `set_port_owner(fMessagePort, fClientTeam)` is the crash isolation pattern. MessageLooper.cpp lines 140-164 — error from GetNextMessage breaks the loop, doesn't crash.

**Why:** The 3-week cost delta (custom 5-6 weeks vs par-bridge 2-3 weeks) pays back immediately — every protocol in Phase 3+ inherits the transport, crash handling, and calloop integration.

**How to apply:** This is the Phase 2 recommendation. If the designer agrees, next step is the typestate prototype. If timeline pressure forces compromise, par-for-specification + hand-written-state-machine is the fallback.
