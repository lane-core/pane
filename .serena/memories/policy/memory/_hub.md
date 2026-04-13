---
type: policy
status: current
supersedes: []
created: 2026-04-11
last_updated: 2026-04-12
importance: high
keywords: [memx, memory, serena, discipline, principles]
related: [reference/papers/memx, MEMORY]
agents: [all]
---

# Memory discipline

Reference for organizing serena memory faithful to MemX findings
(Sun, March 2026), adapted for serena name-based retrieval.

## What MemX is

Local-first hybrid retrieval: vector search (DiskANN) + keyword
(FTS5), fused w/ Reciprocal Rank Fusion (k=60), re-ranked by
four factors (semantic 0.45, recency 0.25 / 30-day half-life,
frequency 0.05 log-normalized, importance 0.10), z-score + sigmoid
normalization, low-confidence rejection when both keyword set empty
AND vector similarity < τ=0.50. Link graph (`memory_links`, 7
relation types) exists in data model but not yet in search pipeline.

Key empirical findings: (1) semantic density per record = primary
retrieval quality driver — fact-level chunking doubles Hit@5 vs
session-level, gap widens at scale. (2) Dedup data-dependent — helps tagged template data, hurts tag-free atomic
facts. (3) One store per user, not per app/agent.

## What serena is

Flat namespace of named markdown files, exact-name retrieval. No
vector index, FTS, fusion, re-ranking, rejection, or link metadata.
Agent calls `list_memories()`, picks names, calls `read_memory(name)`.

Scoped to project, not agent. Every agent reads/writes same store.
Port of MemX "one store per user" to multi-agent: one store per
project, all agents share.

## The mapping

| MemX mechanism | Serena equivalent |
|---|---|
| Vector retrieval (semantic) | Semantic similarity in memory **names** |
| Keyword retrieval (FTS5) | Identifiers in memory **content** that agent can grep |
| Four-factor reranking | Auto-memory `MEMORY.md` index ordering |
| Recency factor | Manual `last_updated` frontmatter |
| Frequency factor | Manual `importance` frontmatter (proxy for citation frequency) |
| Importance factor | Same — explicit annotation |
| Supersession links | Manual `supersedes` / `superseded_by` frontmatter |
| Related / extends / contradicts links | Manual frontmatter fields w/ same names |
| Low-confidence rejection | Agent discipline: say "no relevant memory" instead of stretching |
| Access vs retrieval separation | Which memories get *cited* vs read for context |
| One store per user | One store per project, all agents share |

## Spokes

- [policy/memory/principles](policy/memory/principles) — 10 ported principles (granularity, frontmatter, index, namespace, etc.)
- [policy/memory/agents](policy/memory/agents) — read-everywhere/write-own-folder, shared-vs-private rule
- [policy/memory/hub_spokes](policy/memory/hub_spokes) — when/how to use hub-and-spokes pattern
- [policy/memory/schemas](policy/memory/schemas) — per-type memory layouts + anti-patterns
- [policy/memory/workflow](policy/memory/workflow) — rules for every read/write/merge
- [policy/memory/migration](policy/memory/migration) — restructuring discipline, forwarders, staleness prevention