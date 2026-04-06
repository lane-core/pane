# Spec Fidelity Audit (2026-04-05)

Audit at 93 tests (updated 2026-04-05; was 97 at initial audit), commit e185898. docs/architecture.md vs implementation.

## Critical divergences (spec misleads readers)

1. **Handler trait:** spec shows 8 methods, implementation has 5. Missing: pane_exited, supported_properties (uses PropertyInfo which doesn't exist), request_received.
2. **PaneEntry::update_state:** spec says &self with ArcSwap, implementation is &mut self with direct assignment. ArcSwap not integrated.
3. **AttrSet<S> and AttrReader<S>** exist in BOTH pane-proto/src/monadic_lens.rs AND pane-fs/src/attrs.rs with different semantics. Name collision.
4. **Closure form Pane::run:** spec example shows 1-param closure, implementation takes 2 params (Messenger + LifecycleMessage).
5. **Wire protocol:** spec says 256-slot ceiling, implementation caps at 255 (0xFF reserved).

## Stale content

- property.rs Attribute<'a,S,A> backed by fp_library is dead code — MonadicLens supersedes it
- PropertyInfo referenced in spec doesn't exist; AttrInfo is the implemented successor
- WriteError::NotFound in optics-design-brief not in implementation (only ParseError, ReadOnly)

## Well-aligned areas

- Session types (ClientHandshake, ServerHandshake), Message trait, Protocol+ServiceId, Handles<P>, Flow, obligation handles, FrameCodec, destruction sequence, ExitReason all match spec closely
- Invariants I1/I4/I9/I10/I11/I12/S1/S4/S5 validated by tests
- dispatch_ctl matches spec's optic-routed-with-fallback design

**How to apply:** When verifying against architecture.md, cross-check Handler trait, PaneEntry snapshot model, and AttrSet naming against actual code. Don't trust the Handler method listing — it's stale.