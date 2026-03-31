# BeAPI Naming Policy

**Full guide:** `docs/naming-conventions.md` — decision tree, method patterns, divergence protocol.

**Summary:** Three tiers, applied in order:

1. **Faithful adaptation** (default) — Be name, drop B prefix, Rust case conventions.
2. **Justified divergence** — new name when concept is architecturally different. Requires divergence tracker entry.
3. **Rust idiom** — standard patterns (iterators, Result, From/Into, builders) when Rust has an established convention Be lacked.

"Sounds better" is not a reason. Ask "what did Be call this?" before coining anything.

Early design decisions are NOT immutable. Rename now while the cost is zero.