Here's the compressed version:

---
type: policy
status: current
created: 2026-04-11
last_updated: 2026-04-12
importance: normal
keywords: [hub, spokes, cluster, navigation, overview]
extends: policy/memory/_hub
agents: [all]
---

# Hub-and-spokes pattern

For analysis clusters w/ 4+ related memories. Overview memory
orients agent before descending into spokes. Justified by
fact-level-granularity (smaller memories retrieve better) +
navigation cost of too many orphaned facts.

## Break-even point is ~4 spokes

Below 3: keep flat. Above 4: hub-and-spokes wins. At 3: judgment
call. Cost model: flat read costs `r·log₂(N)` to find right spoke;
hub-and-spokes costs `r_overview + r_spoke + log₂(N)`. Overhead
pays when `r_overview < (N−1)·p_waste·r_spoke`. Lands near 4.

## Hubs reference, never contain

**Hub does not mirror spoke content.** Hub orients (why cluster
exists, how spokes relate, read order) + points (one-line hooks).
Does NOT copy content. Spoke updated → content-mirroring hub lies.

Structural reason: hub has editorial content not recoverable from
spokes — slice category apex, not coproduct. Spokes write into hub
via `extends: <hub_path>` in own frontmatter; hub maintains pointer
list but doesn't own spoke content.

Same rule applies to top-level project index (`MEMORY`): it's a
hub. Orients agents to query categories, points at memories w/o
containing them.

## Hub naming

Use `_hub.md` (or `_index.md`) inside cluster folder. Underscore
prefix sorts first in dir listings. Precedent: Haiku's
`reference/haiku-book/app/_app_intro.dox`. Multiple `overview.md`
files across folders = navigation hazard.

## Inside the hub

Minimal structure (~150 lines max):
- **Motivation** — why cluster exists, what bound gaps together
- **Spokes** — flat list w/ one-line hooks (only place spoke
  content appears, one-line form only)
- **Open questions** — unresolved issues
- **Cross-cluster references** — citations into other hubs

## When NOT to use

- Clusters of 3 or fewer
- Tightly bound content where every part requires others (split
  forces every spoke read to also read hub — overhead exceeds lift)
- Volatile clusters churning weekly (hub stales faster than earns)
- Singletons (top-level files, not folders)