Here's the compressed version:

---
type: policy
status: current
created: 2026-04-11
last_updated: 2026-04-12
importance: normal
keywords: [schemas, types, layout, anti-patterns]
extends: policy/memory/_hub
agents: [all]
---

# Per-type memory schemas + anti-patterns

## Schemas

Skeletal — enough to know what section to write in.
Be Inc. docs philosophy: dev who knows "Hook Functions"
on BView page knows where on BWindow page.

- **status.md** (singleton): `## Where we are` → `## What's next` → `## Known open questions`
- **decision/<topic>.md**: `## Decision` (one sentence) → `## Why` (provenance) → `## Consequences` (forbids/enables)
- **architecture/<subsystem>.md**: `## Summary` → `## Components` → `## Invariants` → `## See also`
- **analysis/<topic>/_hub.md**: `## Motivation` → `## Spokes` → `## Open questions` → `## Cross-cluster references`
- **analysis/<topic>/<spoke>.md**: `## Problem` → `## Resolution` → `## Status`
- **policy/<rule>.md**: `## Rule` → `## Why` → `## How to apply`
- **reference/<source>.md**: free-form (pointers, not structured)
- **agent/<n>/<topic>.md**: free-form (institutional knowledge shapes to content)

Hub vs spoke layout asymmetry load-bearing — Be Book had
different layouts for `BHandler_Overview.html` (prose) and
`BHandler.html` (reference). Same shape both destroys
conceptual-vs-mechanism distinction.

## Anti-patterns

- **Per-agent store parallel to project store.** Knowledge fragments by tool; cross-agent visibility lost.
- **Dated session memories as peers.** `subsystem_session_YYYY_MM_DD` alongside `subsystem_current`. Snapshots → `archive/`.
- **Omnibus memories.** Seven headers each describing separate decision. Split.
- **Cross-project orphans.** Memory from project A in project B namespace. Move it.
- **Sister namespaces.** `policy/` alongside `project/policy/`. Pick one.
- **Hub mirroring spokes.** Hub copying spoke content goes stale on spoke update.
- **Folder with one required file.** Invites dated-peer anti-pattern. Use top-level file.
- **Flat archive without structure.** `archive/x.md` with no provenance. Shadow live structure.
- **Indexes mirroring `list_memories()`.** Flat re-statement of names without query-type grouping.
- **Implicit supersession.** New memory replaces old without frontmatter. Next session can't tell.
- **Stretching low-relevance memories.** Citing tangential match instead of "no relevant memory."
- **Cross-agent writes.** Agent A editing `agent/B/`. Use `supersedes:` / `contradicts:` from own folder.