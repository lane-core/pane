---
name: Architecture spec fidelity audit (2026-04-05)
description: Audit of docs/architecture.md against implementation at commit e185898. 97 tests. 5 critical findings, 2 stale items, name collision across crates.
type: project
---

Audit at 97 tests, commit e185898. Key findings:

**Critical divergences (spec misleads readers):**
1. Handler trait: spec shows 8 methods, implementation has 5. Missing: pane_exited, supported_properties (uses PropertyInfo which doesn't exist), request_received.
2. PaneEntry::update_state: spec says &self with ArcSwap, implementation is &mut self with direct assignment. ArcSwap not integrated.
3. AttrSet<S> and AttrReader<S> exist in BOTH pane-proto/src/monadic_lens.rs AND pane-fs/src/attrs.rs with different semantics. Name collision.
4. Closure form Pane::run: spec example shows 1-param closure, implementation takes 2 params (Messenger + LifecycleMessage).
5. Wire protocol: spec says 256-slot ceiling, implementation caps at 255 (0xFF reserved).

**Stale content:**
- property.rs Attribute<'a,S,A> backed by fp_library is dead code — MonadicLens supersedes it
- PropertyInfo referenced in spec doesn't exist; AttrInfo is the implemented successor
- WriteError::NotFound in optics-design-brief not in implementation (only ParseError, ReadOnly)

**Well-aligned areas:**
- Session types (ClientHandshake, ServerHandshake), Message trait, Protocol+ServiceId, Handles<P>, Flow, obligation handles, FrameCodec, destruction sequence, ExitReason all match spec closely
- Invariants I1/I4/I9/I10/I11/I12/S1/S4/S5 validated by tests
- dispatch_ctl matches spec's optic-routed-with-fallback design

**Why:** Needed to know where the spec can be trusted vs where it's aspirational or stale.
**How to apply:** When verifying against architecture.md, cross-check Handler trait, PaneEntry snapshot model, and AttrSet naming against actual code. Don't trust the Handler method listing — it's stale.
