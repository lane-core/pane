---
type: policy
status: current
created: 2026-04-11
last_updated: 2026-04-12
importance: normal
keywords: [migration, restructure, forwarder, staleness, merge, archive, tiers]
extends: policy/memory/_hub
agents: [all]
---

# Migration discipline

## Restructuring order

1. **Inventory and triage first.** Categorize every memory:
   stay-as-is / rename-and-move / split-into-cluster / archive /
   forwarder / delete. Do before touching files. Consistency audit
   after triage catches misclassifications.
2. **Write new index next.** Cheapest op, validates structure
   before moves.
3. **Migrate highest-impact first.** Status, most-read policies,
   most-cited analyses. Each batch improves store immediately.
4. **Forwarders during cycle.** Renamed memory leaves stub at old
   path. Un-migrated cross-refs keep resolving.
5. **Sweep forwarders next migration.** One cycle enough for
   stale refs.
6. **Consistency audit at end.** No broken links, orphaned
   forwarders, frontmatter-less memories, or duplicates.

## Forwarder template

```
See `<new_path>`.
```

Optional frontmatter:

```yaml
---
type: forwarder
superseded_by: <new_path>
---
```

Forwarder job: make `read_memory(old_name)` return useful
pointer. Nothing beyond redirect.

## Multi-layer migrations

Layer retiring → no forwarders in it. Update only top-level index
pointing at surviving layer. Individual files vanish with layer.

Persisting layer pairs → forwarder in each old path pointing at
canonical new location.

## Naming when merging across layers

Merged result uses canonical layer's name unless both layers
consistently used prefix. Canonical = authoritative source going
forward (usually surviving layer). Provenance from non-canonical
name in `supersedes:` frontmatter. Prefer names without
layer-specific provenance markers.

## Archive only when snapshot has independent value

Don't archive just because overwriting — git preserves old content.
Archive to `archive/<type>/<date>.md` only when snapshot has
historical value beyond merged result (e.g., status at handoff
point). Policy merges where merged version = complete superset
don't need archives. When in doubt: don't archive.

## Hand-merge pattern

1. Identify more developed version (longer, more examples).
2. Use as base.
3. From other version, fold in only **unique content**: provenance,
   examples, framings not in base.
4. Record both source paths in `supersedes:`.
5. Don't interleave sentence-by-sentence — reads worse than either.

Both equally developed + say different things → not duplicates.
Apply merge test (§7): would agent searching for X land on both?

## Staleness prevention

Five staleness vectors: source drift between read/write,
external sources moved on, cross-cluster refs stale,
forgotten unique content, git/serena divergence.

### Tier 1: per-merge (every write)

- Re-read sources at write time, not plan time.
- Verify external sources for status/architecture (re-check
  `PLAN.md`, `git log`, code).
- Set `last_updated` to merge time, not plan time.
- Use `verified_against:` and `sources:` frontmatter:
  ```yaml
  sources: [pane/old_name, auto-memory/feedback_old_name]
  verified_against: [PLAN.md@HEAD, commits-since-2026-04-06]
  ```

### Tier 2: post-merge audit (per phase)

- **Pointer resolution** — verify all `related:`, `extends:`,
  `supersedes:` targets exist.
- **Diff against sources** — every claim traces to named source.
- **Cross-cluster grep** — nothing points at old path
  without forwarder.

### Tier 3: periodic re-verification

- **`last_updated` triage** — every N sessions, list memories
  >14 days old. Still current → bump timestamp; stale → mark
  `needs_verification`.
- **External source change detection** — major code change →
  identify affected architecture/decision memories → mark
  `needs_verification`.
- **`verified_against:` audit** — old timestamps = due for
  re-verification.

`needs_verification` = soft supersession: still serves reads,
readers see flag, treat claims as provisional.

## When MemX gets richer affordances

Vector index/FTS5 → `keywords` becomes redundant, `last_updated`
redundant under automatic recency. But `supersedes`/`superseded_by`
still needed (MemX's own link graph not search-integrated).

Hub-and-spokes, per-type schemas, `agent/<n>/` discipline remain
useful even with automatic retrieval — organizational disciplines,
not retrieval workarounds. Keep store legible to humans auditing
agent work.