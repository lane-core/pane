---
name: Vector similarity design decisions for pane-store
description: Key architectural decisions for vector similarity in pane-store (2026-03-26): HNSW in pane-store itself, threshold-based live queries, embedding xattr format, model migration protocol
type: project
---

Vector similarity search lives in pane-store alongside the attribute index, not as a separate service or in the kit or FUSE layer.

**Why:** BFS principle -- one indexing service, one query interface. BLooper self-contained semantics means pane-store owns the "queries over files" domain completely. Splitting vector into a separate service breaks fusion (two services for one query) and live query notification flow.

**How to apply:**
- pane-store maintains two indices: attribute (predicate) and HNSW (vector)
- Both updated from same fanotify event loop
- Query API uses builder pattern with `.predicate()` and `.similar_to()` -- hybrid queries compose both
- Live queries for vector component use THRESHOLD (not top-k) to preserve O(1) per-change evaluation. Top-k only available for one-shot queries.
- Embedding xattr format: `[u16 dim][u64 model_hash][f32*dim vector]`
- Model migration is gradual: new HNSW index on first new-model embedding, stale set shrinks as files re-embed
- Standard dimensionality: 1024 (Qwen3-0.6B native)
- Key Haiku source reference: `QueryParser.h` `_EvaluateLiveUpdate()` (line ~1639) shows the differential evaluation pattern that makes predicate live queries O(1). Threshold vector queries preserve this property.

Research document: `/Users/lane/src/lin/pane/openspec/changes/spec-tightening/research-vector-similarity-design.md`
