---
type: decision
status: current
supersedes: [pane/panefs_query_unification]
sources: [pane/panefs_query_unification]
created: 2026-03-22
last_updated: 2026-04-11
importance: high
keywords: [pane_fs, query, namespace, BFS, plan9, computed_views, by_sig, by_type, projection]
related: [decision/observer_pattern, decision/host_as_contingent_server, reference/plan9/divergences, reference/plan9/man_pages_insights]
agents: [pane-architect, plan9-systems-engineer, be-systems-engineer]
---

# pane-fs as unified query / namespace system

pane-fs's directory hierarchy is the unification of BFS queries
and Plan 9 synthetic filesystems:

- **BFS:** typed attributes → indices → live queries → dynamic
  result sets
- **Plan 9:** synthetic filesystems → computed views → namespace
  composition
- **pane-fs:** directory hierarchy IS the query system. Each
  directory is a view — a projection of pane state through a
  filter.

## Examples

- `/pane/by-sig/com.pane.agent/` = BFS query
  `signature == 'com.pane.agent'` expressed as a path
- `/pane/remote/` = `topology == remote`

Every level of nesting adds a filter. The tree is computed,
not stored.

## Local + remote unified

The unified namespace (local + remote interleaved) is the
**default**. Filtering to local-only or remote-only is just
another computed directory view. `local` / `remote` is one more
predicate in the same system as `by-sig`, `by-type`, `by-user`.

## Two-tier model

The metadata layer (pane-store attributes) provides the **data**.
pane-fs provides the **projection**. Same system at different
tiers.
