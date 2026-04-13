---
type: policy
status: current
created: 2026-04-11
last_updated: 2026-04-12
importance: high
keywords: [workflow, write, read, merge, frontmatter, rules]
extends: policy/memory/_hub
agents: [all]
---

# Memory workflow rules

1. **On every write** — check topic has memory. Yes → update; superseded → link explicitly.
2. **On every write** — populate frontmatter. Skip only ephemeral working notes.
3. **On every read for a task** — check frontmatter. `status: superseded` → follow `superseded_by`. `status: needs_verification` → treat provisional, re-verify before citing.
4. **On every merge** — re-read all sources immediately before writing (not planning-time content). Verify external authoritative sources (`PLAN.md`, `TODO.md`, `git log`, code) for status/architecture memories. Record in `verified_against:`.
5. **On significant state changes** — update index first, then affected memories. Index cheapest to fix.
6. **On consolidation/deletion** — apply merge test before merging, citation-count proxy before deleting.
7. **On agent boundary writes** — write only to own folder under `agent/<n>/`. Cross-agent supersession/contradiction: write in own folder, use `supersedes:` / `contradicts:` frontmatter pointing at other agent's memory.
8. **On migrations** — leave thin forwarder stubs at old paths one cycle, sweep next migration. Forwarder = one-line redirect: `See <new_path>.`. Preserves cross-references during cutover.