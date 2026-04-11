---
name: Pane-as-trait debate — Plan 9 vs Be positions
description: Structured defense and counter-attack on Handler restructuring — concessions, strong points, and unresolved tensions (2026-03-31)
type: project
---

Formal debate between Plan 9 and Be engineer positions on Pane-as-trait (2026-03-31).

## Be critiques addressed

1. **Binding time (compile vs runtime):** Conceded that literal mount/bind analogy is wrong. Defended that the *startup-time namespace composition pattern* is the real analogy, not the runtime mutability. Confidence: high.

2. **Redundant systems (trait + protocol caps):** Acknowledged redundancy is real. Counter: Plan 9 had the same pattern (auth library + factotum filesystem). Two layers catch different error classes. Whether cost exceeds value depends on implementation complexity. Confidence: medium.

3. **Cross-cutting concern bypass:** Conceded that bypassing the Message/FilterChain pipeline would be a regression. Defended: Phase 3 design already specifies batch unification (clipboard events enter as Message variants). Trait dispatch happens *after* filter chain, not instead of it. Open question: what happens when a clipboard Message reaches a handler that doesn't implement ClipboardPane? Confidence: high on addressability, medium on elegance.

4. **Premature crystallization:** Conceded the timing risk. Counter: Handler already has 3 clipboard methods — crystallization exists. Pane-as-trait makes existing crystallization legible. Aligned with prior recommendation: build clipboard first, extract patterns after observer.

## Attacks on Be position

1. **N+1 handler growth:** Each service adds methods to Handler. Plan 9 split by file (proc/ctl, proc/status, proc/note), not by method bag. rio split snarf/mouse/wctl/cons. The flat Handler model doesn't scale.

2. **"Open what you use" principle violated:** Current model delivers all event types to all handlers. Plan 9's per-file opt-in lets processes (and the system) know what a consumer cares about. Matters for distribution (don't route clipboard traffic to panes that don't use clipboard).

3. **"Don't fix what works" is the factotum trap:** Plan 9's own history — embedded auth "worked" until it didn't. Factotum's separation of concerns looked like over-engineering until protocol changes required it. Same dynamic applies to Handler + services.

4. **Single-port property is contingent:** Phase 3 batch unification works only as long as all sources produce LooperMessage::Posted. First source that needs bypass (latency-sensitive response) breaks it. rio's separate blocking files are the mature version of this.

5. **Distribution requirements already push toward structuring:** Compositor needs to know pane capabilities, protocol needs negotiation, service disconnect needs per-service handling. All three are on the roadmap.

## Unresolved

- Looper specialization mechanism (how to detect trait bounds without reflection) remains the hard technical question
- Whether the concrete cost of two systems (trait + protocol caps) exceeds the bugs they catch — needs implementation to evaluate
- Closure `pane.run()` ergonomics under Pane-as-trait

**Why:** Lane asked for structured defense/attack on the two engineering positions.

**How to apply:** Reference when the Pane-as-trait decision is made. Key concessions and open questions recorded here should inform the final design choice.
