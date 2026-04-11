---
type: policy
status: current
supersedes: [naming/beapi_naming_policy, auto-memory/feedback_beapi_naming_policy]
created: 2026-03-29
last_updated: 2026-04-10
importance: high
keywords: [beapi, naming, faithful_adaptation, divergence, rust_idiom, B_prefix]
agents: [pane-architect, be-systems-engineer]
---

# BeAPI Naming Policy (CRITICAL)

**Rule:** pane API naming defaults to faithful BeOS convention. Deviations require explicit justification.

**Full guide:** `docs/naming-conventions.md` — decision tree, method patterns, divergence protocol.

## Three tiers, applied in order

1. **Faithful adaptation** (default) — Be name, drop B prefix, Rust case conventions.
2. **Justified divergence** — new name when concept is architecturally different. Requires divergence tracker entry.
3. **Rust idiom** — iterators, Result, From/Into, builders when Rust has an established convention Be lacked.

"Sounds better" is not a reason for tier 2.

**Why:** Adopting Be conventions faithfully prevents reinventing naming that Be already got right. The API is in initial development — nothing is immutable. Rename now while the cost is zero.

**How to apply:** Before naming anything, ask "what did Be call this?" If Be had it, use that name (tier 1). If the concept diverges architecturally, coin a new name and record it (tier 2). If Rust has a standard pattern, use it (tier 3). See `docs/naming-conventions.md` for method naming patterns and the full decision tree.
