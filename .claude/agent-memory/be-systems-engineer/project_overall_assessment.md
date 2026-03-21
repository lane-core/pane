---
name: Overall spec assessment (2026-03-20)
description: Be engineer's final assessment of foundations + architecture specs — design sound, key risks are optics concreteness, AI Kit scope creep, and bus factor
type: project
---

Assessment of both specs after full review cycle (foundations spec + architecture spec post-rewrite).

**Verdict:** Design is sound and buildable. Foundations spec is excellent — captures "why" not just "what." Architecture spec is Phase 1-2 ready, Phase 3-4 mostly ready, Phase 5+ intentionally deferred.

**Three key risks not in the open questions:**
1. **Optics still aspirational** — no concrete Rust type for state-to-view lens. Need a spike in Phase 3 to prove static optics are practical before Phase 6 dynamic composition.
2. **AI Kit doing too much conceptual work** — risk of agent vision driving architectural decisions that don't serve the non-agent case. Advice: build the desktop, agents follow.
3. **Bus factor of one** — need second contributor before Phase 4.

**Phase 2 caveat:** Spec describes target and fallback but not acceptance criteria. Need latency/throughput/concurrency benchmarks defined before prototyping begins.

**What would make this succeed:**
- pane-shell-in-pane-comp by month four (dogfood feedback loop)
- Second person before Phase 4 (accountability, not just code)
- AI Kit deferred until desktop works
- Ship incomplete things that work — each phase is a proof, not a spec exercise

**Why:** Gassée's principle — "the spec is magnificent, now show me a demo." Every desktop project since BeOS that died, died in the long tail of boring integration problems, not because the architecture was wrong.

**How to apply:** Treat this as the project's strategic checkpoint. Reference when scope creep threatens or when the temptation to keep specifying outweighs shipping.
