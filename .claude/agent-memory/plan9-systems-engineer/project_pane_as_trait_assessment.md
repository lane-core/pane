---
name: Pane-as-trait assessment
description: Plan 9 analysis of making Pane a trait with service extension traits (ClipboardPane, etc.) — approved with caveats
type: project
---

Pane-as-trait consultation (2026-03-31).

## Decision: Pane-as-trait is sound, with caveats

Lane proposed making Pane a trait (app type IS the pane) with service capabilities as extension traits (ClipboardPane, ObserverPane, etc.). Five questions answered from Plan 9 perspective.

## Key findings

1. **Static capabilities acceptable.** Plan 9 namespaces are technically mutable post-startup, but in practice services were in the namespace at startup and stayed. Static trait-based channel wiring matches the common Plan 9 pattern. Cost of idle channel is negligible.

2. **Not an fd table.** Pane-as-trait is compile-time service dispatch, not a generic registry. Consistent with prior recommendation ("no fd table, build concrete clipboard, not generic framework"). Watch for premature generalization into `ServicePane<S>`.

3. **Protocol-level capabilities needed separately.** Trait bounds tell the local looper what to wire. Remote compositors need independent capability negotiation in the protocol. The trait model doesn't cover this and shouldn't try to.

4. **Looper specialization mechanism is the open risk.** Rust can't reflect on trait bounds. Options: (a) method on base trait returning flags, (b) generic run_pane with specialization, (c) TypeId/Any. (b) is cleanest but combinatorial. If this requires ugly hacks, the simpler `enable_clipboard()` builder approach may win.

5. **Closure run() path needs attention.** Current `pane.run(closure)` disappears or changes fundamentally when the type IS the pane. Hello-world ergonomics must stay simple.

**Why:** Lane asked whether Pane-as-trait serves the architecture vs. introducing static rigidity where Plan 9 uses dynamic composition.

**How to apply:** Reference when implementing Phase 3 channel topology. Resist generic ServicePane<S> trait. Ensure protocol-level capability story exists alongside trait-based wiring. Verify looper specialization mechanism before committing.
