---
type: policy
status: current
supersedes: []
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [memx, memory, serena, discipline, principles, hub_spokes, frontmatter, namespace, migration, forwarders, archive, agent_folders, staleness, merge_test, low_confidence_rejection]
related: [reference/papers/memx, MEMORY]
agents: [all]
---

# MemX principles, ported to serena

A reference for organizing serena memory in a way that's faithful to
the empirical findings of the MemX paper (Sun, March 2026), adapted
for serena's name-based retrieval architecture.

## What MemX actually is

MemX is a local-first hybrid retrieval system for AI assistant
memory. It runs vector search (DiskANN over dense embeddings) and
keyword search (FTS5 full-text) in parallel over the same corpus,
fuses the ranked lists with Reciprocal Rank Fusion (k=60), re-ranks
by four factors (semantic 0.45, recency 0.25 with 30-day half-life,
frequency 0.05 log-normalized, importance 0.10), normalizes with
z-score + sigmoid, and applies a low-confidence rejection rule that
returns ∅ when *both* the keyword set is empty *and* vector
similarity falls below τ=0.50. The data model has a `memory_links`
table with seven relation types (similar, related, contradicts,
extends, supersedes, caused_by, temporal), although the paper notes
the link graph is not yet integrated into the search pipeline.

The most important empirical finding is not in the principles
section: **semantic density per record is the primary driver of
retrieval quality at scale**. Fact-level chunking doubles Hit@5
versus session-level chunking on the LongMemEval benchmark, and the
gap widens as the corpus grows. The second-most-important finding
is that **deduplication is data-dependent** — it improves recall on
tagged template data but actively *hurts* recall on tag-free atomic
facts.

A subtler load-bearing point: MemX is **one memory store per user**,
not one per app, not one per agent. The access-vs-retrieval
separation is at the *entry* level (which entries get cited vs which
get read for context), not at any structural boundary above the
entry. This is the unit of consolidation MemX assumes.

## What serena is

Serena memory is a flat namespace of named markdown files retrieved
by exact name. There is no vector index, no full-text index, no
fusion, no automatic re-ranking, no rejection rule, and no link
metadata. The agent calls `list_memories()`, picks names by hand
based on what they look like, calls `read_memory(name)` to retrieve
contents, and synthesizes from there.

A serena memory store is **scoped to a project, not to an agent**.
Every agent that works on the project — whether a design specialist,
a code-writer, a verifier, or an orchestrator — reads and writes to
the same store. There is no parallel per-agent memory layer
alongside serena. This is the natural port of MemX's "one store per
user" model to a multi-agent setting: one store per project, every
agent on the project shares it. Agent-specific institutional
knowledge lives **inside** the store, in a designated sub-namespace,
not in a parallel system.

This is a fundamentally simpler retrieval architecture than MemX.
None of MemX's algorithms transfer directly. But the **design
pressures** that motivate those algorithms still apply, because
without them the agent has to compensate manually.

## The mapping

| MemX mechanism | Serena equivalent |
|---|---|
| Vector retrieval (semantic) | Semantic similarity in memory **names** |
| Keyword retrieval (FTS5) | Identifiers in memory **content** that an agent can grep |
| Four-factor reranking | The auto-memory `MEMORY.md` index ordering |
| Recency factor | Manual `last_updated` frontmatter |
| Frequency factor | Manual `importance` frontmatter (proxy for "how often this gets cited") |
| Importance factor | Same — explicit annotation |
| Supersession links | Manual `supersedes` / `superseded_by` frontmatter |
| Related / extends / contradicts links | Manual frontmatter fields with same names |
| Low-confidence rejection | The agent's discipline to say "no relevant memory" instead of stretching one |
| Access vs retrieval separation | Which memories get *cited* in agent answers vs which get read for context |
| One store per user | One store per project, all agents share it |

## The ported principles

### 1. Fact-level granularity beats session-level

If a memory has internal section headers that an agent might want to
retrieve independently, those sections should be separate memories
with explicit cross-links. Omnibus memories that pack many decisions
into one file measurably reduce retrieval quality even when each
section is well-written. Split when an internal section would be
retrieved independently of the others.

### 2. Frontmatter is the manual reranker

Without automatic scoring, the agent needs explicit metadata visible
on every read. Standard frontmatter:

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

This gives the agent everything MemX computes automatically, and it
makes supersession status visible the moment a memory is opened.
Required for all memories that appear in the index; optional for
working notes.

### 3. The index is the four-factor reranker

The auto-memory `MEMORY.md` (loaded into every conversation) is the
only mechanism that lets an agent navigate without enumerating the
full memory list. It should be **organized by query type, not by
memory name**. The list of memories that exist is what
`list_memories()` is for. The index is what an agent reads first to
decide which memory to retrieve.

A query-organized index looks like:

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

Not like:

```markdown
## All memories
- agent_workflow
- beapi_internals
- clipboard_design_analysis
- ...
```

### 4. Type-aware namespace structure

Replace flat namespaces with typed sub-paths so memories cluster by
how they're queried:

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

The types correspond to query patterns. Status queries go to
`status.md`. Subsystem queries go to `architecture/<subsystem>`.
Per-agent institutional knowledge goes to `agent/<n>/`. The agent
can prune the search space immediately.

Two structural rules:

- **Three-member minimum for sub-folders.** A folder exists when
  there's a meaningful query that benefits from grouping, and that
  query has at least three members to retrieve. Below three, flatten
  with a name prefix (`analysis/optics_scope.md`, not
  `analysis/optics/scope.md` if scope is the only file). This is
  Plan 9's empirical rule for `/sys/src/cmd/`: `ls` is a single
  file, `ip/` is a directory.
- **Status is a top-level singleton, not a folder.** A folder
  containing exactly one required file is a smell — it invites the
  dated-peer anti-pattern (peer status memories from different
  dates). Use `status.md` at the top level; route snapshots to
  `archive/status/<date>.md`.

### 5. Status snapshots are write-once

Status memories track current state. They are updated in place when
the state changes. If a snapshot is needed (for a handoff, for
historical record), it goes to `archive/status/<date>.md` and the
new status memory's frontmatter records `supersedes:
[archive/status/<date>.md]`. Dated peer status memories are an
anti-pattern: they create the impression of multiple authorities
where there should be exactly one.

Status is always at top-level (`status.md`) so the singleton
property is structurally enforced. The archive shadows the live
structure (see §6).

### 6. Archive shadows the live structure

When a memory is archived, its archive path mirrors its live path.
`analysis/eact/gap3.md` becomes `archive/analysis/eact/gap3_<date>.md`,
not `archive/gap3_<date>.md`. The categorical reading: archive is a
full subcategory in the same shape as the live store. The practical
reading: when you restore a memory you don't have to remember where
it came from, and supersession pointers don't dangle.

Archival is **always accompanied by a status flip and pointer
update**, not just a move. The simplest discipline: archive in
place by flipping `status: archived`, and only physically move the
file when storage pressure demands it (which serena will not have
in any realistic project).

### 7. The merge test for apparent duplicates

MemX's deduplication finding: don't merge memories just because
they're on the same topic. The merge test is: **would an agent
searching for content X land on both?** If yes, merge. If no, keep
split and add a `related` link.

Two analysis memories on the same paper that cover different
theorems are NOT duplicates. Two policy memories that say the same
thing in different words ARE duplicates.

### 8. Low-confidence rejection over stretching

When an agent looks for a memory about topic X and finds nothing
directly relevant, the right response is "no memory on this topic"
— not "here's a tangentially related memory I'll stretch to fit."
The MemX rule (R1: reject only when both signals are weak) is the
algorithmic version of this. The serena version is the agent's
discipline.

### 9. Access vs retrieval separation

A memory that gets *cited in answers* (its content informs the
agent's response) is more valuable than one that gets *opened for
context* (read but not used). Without serena tracking this directly,
the proxy is: memories that appear in the index, in agent charters,
or in other memories' `related` fields are high-utility and should
be marked `importance: high`. Memories nobody points to are dead
weight and become candidates for archiving.

## Agents and the project store

The single most important consequence of treating serena as
project-scoped (not agent-scoped) is that **agents share knowledge
through the store**, not through any parallel layer. When the
session-type specialist writes up a protocol analysis, the
implementation specialist can read it. When the verifier finds a
gap, the design agents see it on their next consultation. Without
this consolidation, every agent operates in a private silo and
project knowledge fragments by tool, not by topic.

### Read-everywhere, write-only-to-own-folder

Each agent has a sub-folder under `agent/<agent-name>/`. The
discipline is:

- **Any agent can read any memory in the store**, including other
  agents' sub-folders. Cross-agent visibility is the access pattern
  the consolidated store is for.
- **An agent only writes to its own sub-folder under `agent/`,**
  plus to project-level types (`policy/`, `decision/`,
  `architecture/`, `analysis/`, `reference/`) when the content is
  multi-agent. An agent does not write to another agent's sub-folder.

This mirrors Plan 9's per-user namespace discipline: `/usr/$user/`
is yours to write, `/usr/$other/` is readable but not yours to edit.
If agent A wants to record "agent B's analysis of X is now
superseded by my finding Y," the right move is for A to write its
finding in its own folder and link to B's memory via `supersedes:`
or `contradicts:` in A's frontmatter. The supersession is visible
in both directions because frontmatter is searchable.

### The shared-vs-agent-private rule

A memory lives in `agent/<n>/` only if it is **only useful to that
one agent's institutional knowledge**. Examples of agent-private
content:

- Specific reference passages and their bearing on recurring
  questions only that agent answers
- Cross-references the agent has built between primary sources and
  project decisions
- Recurring misconceptions the agent has corrected within its scope
- The agent's personal "reading order for new sessions"

If multiple agents would consult the same memory, it lives at the
project level instead. Examples of project-level content:

- Process rules every agent follows (`policy/`)
- Design decisions made through multi-agent consultation (`decision/`)
- Subsystem documentation that informs multiple agents'
  recommendations (`architecture/`)
- Audit results that any agent might cite (`analysis/`)

The rule is the access-vs-retrieval principle from MemX applied at
the agent boundary: project-level memory is for retrieval that
crosses agents; agent-level memory is for retrieval that doesn't.

### When an agent consults another agent's memory

The reading discipline mirrors the writing discipline. An agent
reading another agent's memory should treat it as **input to its own
analysis**, not as authoritative for its own scope. If the
session-type specialist reads the optics-theorist's note on monadic
lenses, that's context — the protocol analysis the
session-type specialist produces is its own, written into its own
folder, with the optics note cited via `extends:` or `related:`. The
agents don't merge; they cite.

## The hub-and-spokes pattern

For analysis clusters with 4+ related memories, the hub-and-spokes
pattern adds an overview memory that orients an agent before they
descend into individual spokes. The pattern is justified by the
fact-level-granularity finding (smaller memories retrieve better)
combined with the navigation cost of having too many orphaned facts
to choose between.

### The break-even point is ~4 spokes

Below 3 spokes: keep flat. Above 4: hub-and-spokes wins. At 3:
judgment call. The cost model: a flat read costs `r·log₂(N)` to find
the right spoke; a hub-and-spokes read costs `r_overview +
r_spoke + log₂(N)` decisions. The overhead pays for itself when the
overview eliminates enough wrong spokes that `r_overview <
(N−1)·p_waste·r_spoke`. With realistic constants this lands near 4.

### Hubs reference, they do not contain

The most important rule: **the hub does not mirror the content of
its spokes**. The hub orients (why this cluster exists, how the
spokes relate to each other conceptually, what order to read them
in) and points (one-line hooks per spoke). It does **not** copy
spoke content. The moment a spoke is updated, a content-mirroring
hub lies. The structural reason is that the hub has editorial
content that isn't recoverable from the spokes — it's a slice
category apex, not a coproduct. Operationally: spokes write into the
hub via `extends: <hub_path>` in their own frontmatter; the hub
maintains its own pointer list but does not own the spoke content.

The same rule applies to the top-level project index (often named
`MEMORY`): it is itself a hub. It orients agents to query
categories and points at memories without containing them. When a
new memory is added to a project type folder, the index gains a
one-line entry, never a copy of the content.

### Hub naming

Use `_hub.md` (or `_index.md`) inside the cluster folder, not
`overview.md`. The underscore prefix sorts first in directory
listings, which is the precedent in Haiku's
`reference/haiku-book/app/_app_intro.dox`, `_app_messaging.dox`, etc.
Multiple `overview.md` files across folders all looking the same in
a flat memory list is a navigation hazard.

### Inside the hub

A minimal hub structure (~150 lines max):

- **Motivation** — why this cluster exists, what bound the gaps
  together, who the audience is
- **Spokes** — flat enumerable list of spoke memories with one-line
  hooks (the only place content from spokes appears, and only in
  one-line form)
- **Open questions** — issues the cluster has not yet resolved
- **Cross-cluster references** — citations into other hubs, when
  relevant (these go in the hub body, not only in frontmatter)

### When NOT to use hub-and-spokes

- Clusters of 3 or fewer members
- Tightly bound content where every part requires the others
  (split would force every spoke read to also read the hub for
  context — the navigation overhead exceeds the retrieval lift)
- Volatile clusters where membership churns weekly (the hub goes
  stale faster than it earns its keep)
- Singletons (use top-level files, not folders, see principle 4)

## Per-type memory schemas

Every type has a minimal expected layout. The schemas are
deliberately skeletal — enough to let an agent know what section to
write in, not so much that they constrain content. Be Inc.'s
documentation philosophy applied to memory: a developer who knows
where "Hook Functions" lives on the BView page knows where to find
it on the BWindow page. Same here.

- **status.md** (singleton): `## Where we are` → `## What's next` →
  `## Known open questions`
- **decision/<topic>.md**: `## Decision` (one sentence) → `## Why`
  (provenance: who asked, what alternatives) → `## Consequences`
  (what this now forbids/enables)
- **architecture/<subsystem>.md**: `## Summary` → `## Components`
  → `## Invariants` → `## See also`
- **analysis/<topic>/_hub.md**: `## Motivation` → `## Spokes` →
  `## Open questions` → `## Cross-cluster references`
- **analysis/<topic>/<spoke>.md**: `## Problem` → `## Resolution`
  → `## Status`
- **policy/<rule>.md**: `## Rule` → `## Why` → `## How to apply`
- **reference/<source>.md**: free-form (these are pointers, not
  structured documents)
- **agent/<n>/<topic>.md**: free-form (institutional knowledge
  shapes itself to its content)

The schema asymmetry between hub and spoke is load-bearing — the
Be Book deliberately had different layouts for `BHandler_Overview.html`
(prose, "The Handler List") and `BHandler.html` (alphabetized
reference). Forcing the same shape on hubs and spokes would destroy
the conceptual-vs-mechanism distinction the pattern is built around.

## Anti-patterns

Direct violations of the principles above:

- **Per-agent memory store parallel to the project store.** Agents
  writing to a separate per-tool memory (e.g., `.claude/agent-memory/`)
  alongside serena. Knowledge fragments by tool instead of by topic;
  cross-agent visibility is lost; the principle 9 (one store) is
  violated.
- **Dated session memories as peers.** `subsystem_session_YYYY_MM_DD`
  alongside `subsystem_current`. Snapshots go to `archive/`.
- **Omnibus memories.** A single memory with seven section headers
  each describing a separate decision. Split it.
- **Cross-project orphans.** Memory from project A sitting in
  project B's namespace because they shared a parent during an
  earlier phase. Move it.
- **Sister namespaces parallel to the main one.** A `policy/`
  top-level namespace alongside `project/policy/`. Pick one.
- **Hub memories that mirror their spokes.** A hub that copies
  spoke content goes stale the moment a spoke is updated. Hubs
  reference and orient; they do not contain.
- **Folder containing exactly one required file.** Smells like an
  invitation for the dated-peer anti-pattern. Use a top-level file
  with snapshots in `archive/`.
- **Flat archive without structure.** `archive/x.md`,
  `archive/y.md` with no provenance. Archive should shadow the live
  structure (`archive/analysis/eact/gap3_<date>.md`) so restores are
  trivial and supersession pointers don't dangle.
- **Indexes that mirror `list_memories()`.** A flat index that
  re-states the memory names without grouping them by query type.
  The index loses its purpose.
- **Implicit supersession.** A new memory replaces an old one but
  doesn't say so in frontmatter. The next session has no way to know.
- **Stretching low-relevance memories.** Citing a tangentially
  related memory because it was the closest match, instead of
  saying "no relevant memory found."
- **Cross-agent writes.** Agent A editing a memory in
  `agent/B/`. Use `supersedes:` / `contradicts:` from your own
  folder instead.

## Workflow rules

1. **On every memory write**, check whether the topic already has a
   memory. If yes, update it; if it's been superseded, link the
   supersession explicitly.
2. **On every memory write**, populate frontmatter. Skip only for
   ephemeral working notes that won't be retrieved.
3. **On every memory read for a task**, check the frontmatter. If
   `status: superseded`, follow `superseded_by` and read that
   instead. If `status: needs_verification`, treat the claims as
   provisional and re-verify before citing.
4. **On every merge**, re-read all sources immediately before
   writing the merged result, not from planning-time content.
   Verify external authoritative sources (`PLAN.md`, `TODO.md`,
   `git log`, code) for status and architecture memories. Record
   what you checked in `verified_against:` frontmatter.
5. **On significant project state changes**, update the index
   first, then the affected memories. The index is the cheapest
   thing to fix.
6. **On consolidation or deletion**, apply the merge test before
   merging and the citation-count proxy before deleting.
7. **On agent boundary writes**, only write to your own folder
   under `agent/<n>/`. To record cross-agent supersession or
   contradiction, write a memory in your own folder and use
   `supersedes:` / `contradicts:` frontmatter pointing at the other
   agent's memory.
8. **On migrations**, leave thin forwarder stubs at old paths for
   one cycle, then sweep on the next migration. A forwarder is a
   one-line redirect: `See <new_path>.`. The forwarder construction
   preserves cross-references during the cutover; sweeping prevents
   long-term drift.

## Migration discipline

When restructuring an existing memory store, the order matters:

1. **Inventory and triage first.** Categorize every existing memory
   as stay-as-is / rename-and-move / split-into-cluster / archive /
   forwarder / delete. Do this before touching files. A consistency
   audit pass after the triage catches misclassifications.
2. **Write the new index next.** The index is the cheapest thing
   and validates the new structure before any memories move.
3. **Migrate highest-impact memories first.** Status, the most-read
   policies, the most-cited analyses. Working out from there means
   every batch you complete improves the store immediately.
4. **Forwarders during the migration cycle.** Each renamed memory
   leaves a stub at its old path: `See <new_path>.`. Cross-references
   from un-migrated memories continue to resolve.
5. **Sweep forwarders on the next migration.** One cycle is enough
   to catch any references that needed updating; longer is just
   accumulated dead weight.
6. **Consistency audit at the end.** Walk the new tree and verify:
   no broken links, no orphaned forwarders past their cycle, no
   memories without required frontmatter, no duplicates that
   survived the merge test.

### Forwarder template

A forwarder is a one-line redirect at an old path. Minimal content:

```
See `<new_path>`.
```

Frontmatter is optional for forwarders. When included, only two
fields are useful:

```yaml
---
type: forwarder
superseded_by: <new_path>
---
```

The forwarder's job is to make `read_memory(old_name)` return a
useful pointer. Anything beyond the redirect is overhead. Don't
write a long explanation in a forwarder — the destination has the
explanation.

### Multi-layer migrations

When migrating across layers (e.g., Claude Code auto-memory +
serena project memory + per-agent memory), one layer is usually
being retired. **Don't write forwarders in the retiring layer** —
its index becomes the forwarder. Update only its top-level index
to point at the surviving layer, and leave the individual files
alone. They vanish when the layer is retired.

For pairs across **persisting** layers, write a forwarder in each
old path, pointing at the canonical new location.

### Naming when merging across layers

When two layers had the same content under different names (e.g.,
`pane/agent_workflow` in serena and `feedback_agent_workflow.md`
in auto-memory), the merged result uses the canonical layer's name
unless **both** layers consistently used a prefix (e.g.,
`feedback_*`). The canonical layer is whichever is the
authoritative source going forward — usually the surviving layer.
Provenance from the non-canonical name is preserved in the merged
memory's `supersedes:` frontmatter field, which can list paths
from any layer (`supersedes: [pane/x, auto-memory/feedback_x]`).

When in doubt: prefer the name that doesn't carry layer-specific
provenance markers. The migrated memory should read as a canonical
project memory, not as "the version from layer A merged with the
version from layer B."

### Archive only when snapshot has independent value

Don't archive a memory just because you're overwriting it. Git
history preserves the old content. Archive (to
`archive/<type>/<date>.md`) only when the snapshot has historical
value **beyond** the merged result — for example, a status memory
that captures a project state at a handoff point, where the
specific date and the specific phrasing matter for handoff
continuity.

Policy migrations where the merged version is a complete superset
of both sources don't need archives. Hand-merged analysis memories
where the merge process discarded specific phrasings might.

When in doubt: don't archive. Recovery from git history is
straightforward; cleaning up an over-archived store is annoying.

### Hand-merge pattern for paired memories

Most paired migrations have one "more developed" version (longer,
more examples, more provenance) and one distilled version. The
efficient merge:

1. Identify the more developed version.
2. Use it as the base.
3. From the other version, fold in only **unique content**:
   provenance not present in the base, examples not present in
   the base, alternative framings worth preserving.
4. Record both source paths in `supersedes:` frontmatter.
5. Don't try to interleave the two versions sentence-by-sentence
   — the result reads worse than either source.

If both versions are equally developed and say different things,
that's a signal they're not actually duplicates. Apply the merge
test from §7 of the principles: would an agent searching for X
land on both? If yes, they merge. If no, they should be split with
a `related:` link.

### Staleness prevention during merges

Five ways merges go stale:

- **Source drift between read and write** — a memory updated
  after read, before write, within the same session
- **External sources moved on** — status content based on a
  PLAN.md or code state that's already out of date
- **Cross-cluster references gone stale** — merged memory cites
  `analysis/eact/_hub`, but the hub was renamed in another phase
- **Forgotten unique content** — picked one version as base,
  missed a nugget from the other
- **Git history vs live serena divergence** — committed state
  and working tree diverge

Three discipline tiers prevent and detect this.

#### Tier 1: per-merge (every write)

- **Re-read sources at write time, not plan time.** If you drafted
  the merge during planning, re-read every source immediately
  before issuing the write. Catches drift between planning and
  writing within the same session.
- **Verify external sources for status and architecture memories.**
  For status: re-check `PLAN.md`, `TODO.md`,
  `git log --oneline -10`. For architecture: re-check the code and
  recent commits touching it. Stale architecture memories are the
  highest-risk type because the code has moved underneath them.
- **Set `last_updated` to merge time, not plan time.** The
  frontmatter timestamp tracks freshness; backdating it lies to
  future readers.
- **Use `verified_against:` and `sources:` frontmatter** so the
  next reader can audit what you actually checked. Example:
  ```yaml
  sources: [pane/old_name, auto-memory/feedback_old_name]
  verified_against: [PLAN.md@HEAD, commits-since-2026-04-06]
  ```

#### Tier 2: post-merge audit (per phase)

- **Pointer resolution check.** For each merged memory's
  `related:`, `extends:`, `supersedes:` field, verify the targets
  exist. Broken pointers are detectable mechanically.
- **Diff against sources.** For each merged memory, verify every
  claim traces to at least one named source. The merger should be
  able to point at every paragraph and say which source it came
  from. Gaps = forgotten content.
- **Cross-cluster grep.** For each new memory, grep across all
  other memories for references to its name. If anything still
  points at the old path (and the old path isn't a forwarder),
  the forwarder is missing.

#### Tier 3: periodic re-verification (between phases / sessions)

- **`last_updated` triage.** Every N sessions, list memories with
  `last_updated > 14 days`. For each, re-read and decide: still
  current → bump the timestamp; stale → mark
  `status: needs_verification` and queue for re-merge.
- **External source change detection.** When a major code change
  lands, identify which architecture / decision memories reference
  the changed code. Mark them `needs_verification` and re-merge
  in the next batch.
- **`verified_against:` audit.** Memories with old
  `verified_against:` timestamps are due for re-verification. The
  frontmatter field is the audit signal.

The `needs_verification` status is a soft supersession: the memory
still serves reads, but readers see the flag and know to treat its
claims as provisional. New memories supersede it once verified.

## When MemX gets richer affordances

If serena ever grows a vector index, FTS5, or automatic
reranking, the frontmatter becomes vestigial in some respects but
not in others. `supersedes` and `superseded_by` are still needed —
MemX's own paper notes the link graph is data-model-only, not
search-integrated, even in the published system. `keywords` becomes
redundant under FTS5. `last_updated` becomes redundant under
automatic recency tracking. Design the frontmatter so it's
compatible with future automation: use field names that match MemX's
own data model (`supersedes`, `extends`, `contradicts`, `related`,
`temporal`, `caused_by`, `similar`).

The hub-and-spokes pattern, the per-type schemas, and the
agent/<n>/ sub-namespace discipline remain useful even with
automatic retrieval. They are organizational disciplines, not
retrieval workarounds: they make the store legible to humans
auditing the agent's work, not just to the agent reading it.
