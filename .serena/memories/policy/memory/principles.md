---
type: policy
status: current
created: 2026-04-11
last_updated: 2026-04-12
importance: high
keywords: [memx, principles, granularity, frontmatter, index, namespace, deduplication, rejection, epistemic]
extends: policy/memory/_hub
agents: [all]
---

# Ported MemX principles

### 1. Fact-level granularity beats session-level

Memory has independently-retrievable sections → split w/ cross-links. Omnibus memories reduce retrieval quality. Split when internal section retrieved independently.

### 2. Frontmatter is the manual reranker

No auto-scoring → agent needs explicit metadata. Standard:

```yaml
---
type: status | decision | architecture | policy | reference | analysis | agent
status: current | superseded | archived | needs_verification
supersedes: [memory_name, ...]
superseded_by: memory_name | null
related: [memory_name, ...]
extends: memory_name | null
contradicts: memory_name | null
created: YYYY-MM-DD
last_updated: YYYY-MM-DD
importance: high | normal | low
keywords: [identifier1, identifier2, ...]
agents: [list of agents that consult this]
sources: [memory_name, ...]                            # optional — memories merged into this one (more granular than supersedes)
verified_against: [external_source@version, ...]      # optional — external sources checked at write time (e.g., `PLAN.md@HEAD`, `commits-since-2026-04-06`)
---
```

Required for indexed memories; optional for working notes.

### 3. The index is the four-factor reranker

`MEMORY.md` (loaded every conversation) = only navigation w/o enumerating full list. Organize **by query type, not by memory name**. `list_memories()` gives what exists; index gives what to retrieve first.

Query-organized index:

```markdown
## Start here (every session)
- [status](status.md) — what's done, what's next

## When designing
- [policy/agent_workflow](policy/agent_workflow.md) — the design process

## When working on subsystem X
- [architecture/x](architecture/x.md)

## When you need theoretical grounding for Y
- [analysis/y/_hub](analysis/y/_hub.md)
```

### 4. Type-aware namespace structure

```
status.md         # top-level singleton — exactly one per project
policy/           # process rules, agent workflow, conventions
decision/         # discrete design decisions
architecture/     # subsystem-anchored documentation
analysis/         # theoretical results and audits
  <topic>/       # hub-and-spokes for clusters of 4+ related memories
reference/        # external system documentation
  papers/        # paper anchors (path + summary + concepts informed)
agent/            # per-agent institutional knowledge
  <agent-name>/  # one folder per agent on the project
archive/         # superseded snapshots, shadowing the live structure
```

Two structural rules:
- **Three-member minimum for sub-folders.** Below three → flatten w/ name prefix. Plan 9 empirical rule for `/sys/src/cmd/`.
- **Status is top-level singleton, not folder.** Folder w/ exactly one required file invites dated-peer anti-pattern.

### 5. Status snapshots are write-once

Updated in place on state change. Snapshots → `archive/status/<date>.md`, frontmatter records `supersedes:`. Dated peer status memories = anti-pattern.

### 6. Archive shadows the live structure

`analysis/eact/gap3.md` → `archive/analysis/eact/gap3_<date>.md`. Archive = full subcategory, same shape as live store. Restores trivial, supersession pointers don't dangle.

Archival always w/ status flip + pointer update. Simplest: flip `status: archived` in place, physically move only under storage pressure.

### 7. The merge test for apparent duplicates

Don't merge just b/c same topic. Test: **would agent searching for X land on both?** Yes → merge. No → keep split, add `related` link.

### 8. Low-confidence rejection over stretching

No relevant memory → "no memory on this topic." Don't stretch tangential memory to fit. MemX R1 (reject when both signals weak) = algorithmic version.

### 9. Access vs retrieval separation

Memories cited in answers > memories opened for context. Proxy: memories in index, agent charters, or other memories' `related` fields → `importance: high`. Memories nobody points to → archiving candidates.

### 10. Epistemic strength matches the source

Paraphrase must match source's epistemic strength. Source hedges → memory hedges. "Structurally analogous" ≠ "manifest" or "is." "Not formally verified" caveat must carry through.

**The rule:** Read source's exact wording before paraphrasing. Preserve hedge words/caveats. Don't invent symptoms/examples/narrative source doesn't contain. When in doubt, quote exactly. Tag w/ `verified_against: [<source>@<date>]`.

**Three failure shapes:**
1. **Strengthening a hedge.** Source "structurally analogous" → paraphrase "manifest." Qualifier carried load-bearing uncertainty.
2. **Dropping a caveat.** Source "pattern matches; not formally verified" → paraphrase drops verification caveat.
3. **Inventing detail.** Source names construct but doesn't list failure modes → paraphrase fabricates specifics.

Source's epistemic/detail level = ceiling, never exceeded. `policy/agent_workflow` tier-2 audit enforces this.