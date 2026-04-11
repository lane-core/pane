---
type: reference
status: current
created: 2026-04-10
last_updated: 2026-04-10
importance: high
keywords: [memx, sun, hybrid_retrieval, RRF, vector, keyword, FTS5, four_factor_reranking, low_confidence_rejection, access_retrieval_separation, deduplication, local_first]
related: [reference/papers/_hub, MEMORY, status]
agents: [all]
---

# MemX: A Local-First Long-Term Memory System for AI Assistants

**Author:** Lizheng Sun (March 2026)
**Path:** `~/gist/memx/memx.tex`

## Summary

Local-first hybrid retrieval system for AI assistant memory.
Implemented in Rust on libSQL with an OpenAI-compatible
embedding API. Pipeline:

1. Vector recall (DiskANN over dense embeddings) and keyword
   recall (FTS5) in parallel
2. **Reciprocal Rank Fusion** (k=60) merges the ranked lists
3. **Four-factor re-ranking** (semantic 0.45, recency 0.25 with
   30-day half-life, frequency 0.05 log-normalized, importance
   0.10)
4. Z-score normalization + sigmoid
5. **Low-confidence rejection (R1)** — return ∅ when both
   keyword set is empty AND vector similarity < τ=0.50
6. Two-layer deduplication (content + tag-signature)

Key empirical findings:

- **Semantic density per record is the primary driver of
  retrieval quality at scale.** Fact-level chunking *doubles*
  Hit@5 vs session-level on LongMemEval.
- **Deduplication is data-dependent** — helps tagged template
  data, *hurts* recall on tag-free atomic facts.
- The R1 rejection rule is the only candidate (of five tested)
  with zero false negatives.
- Access vs retrieval separation prevents administrative reads
  from polluting retrieval-based ranking signals.

## Concepts informed

- The principles ported to serena are canonicalized in
  [`policy/memory_discipline`](../../policy/memory_discipline.md):
  fact-level granularity, frontmatter as manual reranker,
  query-organized index, type-aware namespaces, write-once
  status, merge test for apparent duplicates, low-confidence
  rejection, access vs retrieval separation
- One memory store per user / project (the consolidation
  principle)
- The `verified_against:` and `sources:` frontmatter fields
  are pane's manual proxy for MemX's automatic recency and
  link tracking

## Used by pane

- `MEMORY` — the index follows MemX-derived organization
- The whole serena restructure (Phases 1–10) is grounded in
  this paper's principles
- `policy/memory_discipline` is the canonical rulebook
  mapping MemX mechanisms to serena's name-based architecture
