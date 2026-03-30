# MemX and Pane's AI Kit: Local-First LLM Memory

Research analysis of "MemX: A Local-First Long-Term Memory System for AI Assistants" (Sun, March 2026) in the context of pane's agent infrastructure and AI Kit design.

Source: `/Users/lane/gist/memx/memx.tex`

---

## 1. Paper Summary

MemX is a Rust implementation of persistent, searchable memory for conversational AI assistants, built on four design principles: local-first deployment, structural simplicity, real-embedding evaluation, and stability-over-recall.

**The problem it solves.** LLMs are stateless across sessions. Without a memory layer, an assistant cannot retain user preferences, project conventions, incident resolutions, or domain constraints. Existing systems (Mem0, MemGPT) target cloud deployments with end-to-end agent benchmarks. MemX targets the narrower problem of *local* retrieval quality: can you get stable recall of relevant memories while suppressing spurious results when no relevant memory exists?

**Architecture.** Single-file libSQL database (SQLite fork with vector extensions). The search pipeline is deterministic and fixed-stage:

1. **Parallel recall.** Query is embedded via a local or API-compatible embedding model (Qwen3-0.6B, 1024-dim). Two parallel paths: vector recall (DiskANN/brute-force cosine similarity) and keyword recall (FTS5 full-text matching).
2. **Fusion.** Reciprocal Rank Fusion (RRF, k=60) merges the two candidate sets.
3. **Four-factor re-ranking.** Composite score: semantic similarity (0.45) + recency (0.25, exponential half-life decay, 30-day default) + importance (0.10, explicit annotation) + retrieval frequency (0.05, log-normalized).
4. **Z-score normalization + sigmoid.** Makes scores comparable across queries with different candidate-set distributions.
5. **Low-confidence rejection.** If keyword recall is empty AND max vector similarity < threshold tau (default 0.50), return empty. This is the stability commitment: better to return nothing than to hallucinate a memory.
6. **Deduplication.** Two layers -- content dedup (identical trimmed content) and tag-signature dedup (type + sorted tag set). Prevents template-generated clusters from crowding results.
7. **Top-k return + retrieval stats recording.**

**Data model.** Primary `memories` table: content, embedding, type, tags, metadata, importance, plus separated access/retrieval counters and timestamps. Secondary `memory_links` table with seven relation types (similar, related, contradicts, extends, supersedes, caused_by, temporal) -- scaffolding exists but graph traversal is not yet in the search path.

**Key design choice: access vs. retrieval separation.** The system distinguishes "a user explicitly viewed this memory" from "this memory was returned as a search result." Re-ranking uses retrieval counts/timestamps, not access counts. This prevents administrative reads from inflating ranking signals. The paper demonstrates this produces correct ranking reversals compared to access-based tracking.

**Results.** On custom benchmarks (43 queries, up to 1,014 records): Hit@1 = 91-100%, conservative miss suppression. On LongMemEval (500 queries, up to 220,349 records at fact granularity): Hit@5 = 51.6%, MRR = 0.380. Key finding: fact-level storage doubles retrieval quality over session-level storage -- semantic density per record is the primary driver. Temporal reasoning (40.6%) and multi-session reasoning (43.6%) are the weakest categories, requiring mechanisms beyond single-query vector recall.

**Performance.** FTS5 indexing yields 1,100x latency reduction over LIKE-based keyword search at 100k records. End-to-end search under 90ms at 220k records with cached embeddings.

**Known limitations.** Multi-topic coverage gap (single-topic needle-in-haystack only). Graph structure not yet in search path. No temporal indexing. No task-level attribution (whether a retrieved memory was actually *used*). Deduplication is data-dependent: helps with tagged template data, hurts on tag-free atomic facts.

---

## 2. Alignment with Pane

### Where MemX aligns with pane's commitments

**Local-first as design principle, not optimization.** MemX's core thesis -- that a personal AI assistant's memory should live on the user's machine, operate offline, and never require cloud infrastructure -- is exactly pane's commitment. The architecture spec states: "A user running entirely on local models gets the same agent infrastructure" and "The system is designed local-first; remote APIs are an enhancement, not a requirement." MemX validates this position empirically: you can get 91% Hit@1 with a 0.6B parameter embedding model running locally.

**Stability over maximum recall.** MemX's rejection rule -- better to return nothing than to fabricate a memory -- maps directly to pane's design philosophy. The foundations spec's emphasis on correctness models, lens laws, and typed protocols all share the same disposition: don't produce wrong answers just because you can produce fast ones. For agent memory, this means: an agent that says "I don't know" is more trustworthy than one that confidently retrieves a spurious memory.

**Rust implementation.** MemX is written in Rust. Pane is written in Rust. Integration path is direct -- no FFI boundary, no language impedance mismatch.

**Structural simplicity.** MemX is a single-file database with a deterministic pipeline. No distributed systems, no consensus protocols, no eventual consistency. This fits pane's single-machine, single-user-with-agents model perfectly. The system doesn't need to scale to a cluster; it needs to be fast and correct on one machine.

### Where MemX diverges from pane

**MemX has its own storage layer; pane already has one.** This is the central tension. MemX stores everything in a libSQL database -- a monolithic file containing content, embeddings, metadata, link relations, and full-text indices. Pane's architecture commits to filesystem-native state: "everything is a file, queryable via pane-store." Agent mail is files with typed attributes. Agent state is files in home directories. The `.plan` file is a file. The whole system is built around the premise that state lives in the filesystem and is indexed by pane-store's attribute engine.

MemX's libSQL approach is orthogonal to this. It creates an opaque database that pane-store can't see into, that the filesystem projection doesn't reflect, that routing rules can't inspect. It's a black box inside the transparent system.

**MemX assumes a single agent with a single memory store.** The paper is titled "for AI Assistants" -- singular. It models one assistant's memories. Pane models multiple agents (agent.builder, agent.reviewer, agent.researcher, agent.tester, the generalist), each with their own home directory, each with their own `.plan`, communicating over shared infrastructure. The memory question in pane is not "how does *the* agent remember?" but "how does *each* agent maintain state, and how do agents share knowledge when appropriate?"

**MemX doesn't consider permissions or graded access.** The foundations spec's graded equivalence principle -- each observer sees a quotient of the full system determined by their permissions -- has no analogue in MemX. All memories are equally accessible to the one agent. In pane, agent.reviewer should not be able to read memories about the user's personal life that agent.researcher recorded. The grading composes monadically (agent scope intersection namespace isolation intersection Landlock constraints); MemX has no concept of this.

**MemX doesn't consider multi-view consistency.** Pane's isomorphic packaging principle says state must be visible through every view: filesystem, protocol, screen. A memory stored only in a libSQL database violates this -- it's visible through one interface (the MemX API) and invisible to everything else. You can't `ls` an agent's memories, can't query them through pane-store, can't see them in the filesystem projection, can't route them through the routing infrastructure.

---

## 3. What Pane Can Learn from MemX

Despite the architectural divergence, MemX contains several techniques that should directly inform pane-ai's design.

### 3a. Hybrid retrieval is necessary and the split is right

The vector + keyword + RRF fusion pattern is well-validated and directly applicable. Pane-store already commits to a query interface modeled after BQuery with predicate matching. What it doesn't have (and needs for agent memory) is *semantic* search -- vector similarity over embeddings. The hybrid approach MemX validates is exactly what pane-store should grow into:

- **Keyword/attribute matching.** Already in pane-store's DNA (BQuery predicates over typed attributes).
- **Vector similarity.** Needs to be added. Embedding vectors stored as xattr values (or, more practically, in a sidecar index that pane-store manages alongside the attribute index).
- **Fusion.** RRF is simple, parameter-light, and works. No reason to get creative here.

### 3b. The four-factor re-ranking model is sound

Semantic similarity, recency, importance, and frequency -- weighted and z-score normalized -- is a good starting point for memory retrieval. Two observations for pane:

**Recency via filesystem timestamps.** Pane doesn't need to track `last_accessed_at` or `last_retrieved_at` in a database. Files have `atime`, `mtime`, `ctime`. Pane-store already indexes creation and modification time as free attributes. The filesystem provides the recency signal natively. MemX's insight about access/retrieval separation maps to: `mtime` (content changed) vs. `atime` (content read) -- but `atime` is notoriously unreliable on Linux (noatime, relatime). A dedicated `user.pane.last_retrieved` xattr, updated by pane-store when a memory is returned as a query result, is the right translation.

**Importance as an attribute.** This is trivially a pane attribute: `user.pane.importance` with a numeric value. Queryable, indexable, settable by the agent or user.

**Frequency via retrieval count.** Another pane attribute: `user.pane.retrieval_count`. Updated atomically by pane-store on search result return.

### 3c. Storage granularity matters more than pipeline sophistication

This is MemX's strongest empirical finding: going from session-level to fact-level storage doubles retrieval quality (Hit@5: 24.6% to 51.6%), an effect "far larger than any pipeline-component change." The lesson for pane: invest in upstream fact extraction (turning conversations into atomic, queryable statements) rather than building increasingly complex retrieval pipelines.

For pane agents, this translates to a **memory consolidation protocol**: when an agent finishes a conversation or task, it extracts atomic facts and writes them as individual files with typed attributes, rather than storing raw conversation transcripts. The agent's home directory grows a `~/memories/` tree of small, semantically dense files -- each one queryable, each one with attributes, each one visible through the filesystem projection.

### 3d. The rejection rule is essential for trustworthy agents

MemX's low-confidence rejection rule -- return empty rather than return wrong -- is a design principle pane should adopt explicitly. An agent that says "I found a relevant memory" should be right. The conjunctive rule (reject only when BOTH keyword and vector signals are weak) has zero false negatives in MemX's evaluation. This should be a default behavior of pane-ai's memory retrieval, not an optional configuration.

### 3e. The graph scaffolding points the right direction even though MemX doesn't use it yet

MemX defines seven link types (similar, related, contradicts, extends, supersedes, caused_by, temporal) in a `memory_links` table but doesn't yet use them for retrieval. In pane, these relationships are *filesystem relationships*. A file that supersedes another can express this through:
- A `user.pane.supersedes` xattr pointing to the superseded file's path
- A symlink in a `~/memories/.links/supersedes/` directory
- A pane-store query predicate that follows supersession chains

The graph structure MemX leaves as future work is something pane can implement natively through its filesystem infrastructure. Cross-session linking, temporal ordering, contradiction resolution -- these are attribute queries and filesystem relationships, not database joins.

---

## 4. What MemX Misses That Pane Provides

### 4a. The filesystem IS the memory store

MemX builds a purpose-built database because it has no choice -- it's a library running inside an application that has no system-level infrastructure for structured data storage. Pane has pane-store: a system service that indexes typed attributes on files, maintains live queries, and emits change notifications. The entire BFS pattern -- file system as database -- was designed for exactly this kind of problem.

An agent's memory in pane is not rows in a SQLite table. It's files in a directory. Each memory is a file with content and typed attributes (`user.pane.type=memory`, `user.pane.memory_type=procedural`, `user.pane.tags=rust,debugging,borrow-checker`, `user.pane.importance=0.8`, `user.pane.embedding=<binary>`). Pane-store indexes these attributes and answers queries over them. Live queries mean an agent can subscribe to "show me all memories tagged 'debugging' from the last week" and get automatic updates when new memories arrive.

This is what BFS gave BeOS applications for free. Email in BeOS was files with attributes. Contacts were files with attributes. Now agent memories are files with attributes. The pattern works because the infrastructure is general-purpose.

### 4b. Multi-agent memory with graded access

MemX can't even formulate this problem. Pane can solve it through existing infrastructure:

- Each agent has its own home directory; memories in `~/memories/` are owned by that agent
- Filesystem permissions control which agents can read which memories
- Landlock enforcement makes this kernel-guaranteed
- pane-store queries respect filesystem permissions -- an agent can only find memories it can read
- Shared knowledge goes in shared directories (e.g., `/srv/pane/knowledge/`) with appropriate group ownership

The graded equivalence principle applies: each agent sees a quotient of the total memory space, determined by its permissions. The quotient is itself a coherent system -- the agent's view is internally consistent.

### 4c. Memory as routable content

MemX has no concept of routing. In pane, memories are files, files have attributes, and routing rules match on attributes. This means:

- A routing rule can say: "memories with `user.pane.sensitivity=high` never leave the local machine"
- A routing rule can say: "memories tagged `build-result` get forwarded to agent.builder's inbox"
- A routing rule can say: "memories older than 90 days get archived to cold storage"

The routing infrastructure provides memory lifecycle management, inter-agent knowledge sharing, and data governance -- all as declarative rules that the user can inspect and modify. MemX has to build all of this (retention, sharing, governance) as custom application logic. Pane gets it from the infrastructure.

### 4d. Multi-view consistency for memory state

A memory stored as a pane-store-indexed file is automatically visible through:
- **Filesystem:** `ls ~/memories/`, `cat ~/memories/rust-borrow-debugging.md`
- **Queries:** `pane-store query 'type=memory AND tags contains debugging'`
- **Protocol:** Session-typed access through the pane-ai kit
- **Pane projection:** A "memory browser" pane showing memories as a queryable list

MemX's memories exist only through the MemX API. They have no filesystem projection, no protocol representation, no visual representation unless the application builds one. In pane, these projections come for free from the infrastructure.

### 4e. Temporal indexing through filesystem semantics

MemX identifies temporal reasoning as its weakest category (40.6% Hit@5) and calls out temporal indexing as future work. Pane-store already indexes creation time and modification time as free attributes. An agent that writes memories with a `user.pane.event_time` attribute (the time the remembered event occurred, distinct from when the memory was written) gets temporal queries for free: "what happened between March 1 and March 15?" is a pane-store predicate query.

---

## 5. Concrete Recommendations for the AI Kit

### 5a. Agent memory IS filesystem + pane-store. Don't build a separate database.

The temptation will be to embed a vector database (or libSQL, a la MemX) inside pane-ai. Resist this. The architecture already has the right primitives:

**Memory storage:**
- Each agent's memories live in `~agent/memories/`, organized however the agent finds useful (flat, by topic, by date, by type)
- Each memory is a file: content as file content, metadata as typed xattrs
- Standard pane attributes: `user.pane.type`, `user.pane.tags`, `user.pane.importance`, `user.pane.created`, `user.pane.memory_kind` (episodic, semantic, procedural)

**Memory indexing:**
- pane-store indexes all `user.pane.*` attributes automatically
- Add vector embedding support to pane-store: `user.pane.embedding` as a binary xattr containing the embedding vector, with pane-store maintaining a vector index (HNSW or DiskANN) alongside its existing attribute index
- This is the one genuine capability gap: pane-store needs vector similarity search. But this is a pane-store enhancement, not an AI Kit concern. It benefits every component, not just agent memory.

**Memory retrieval:**
- pane-ai provides a memory retrieval function that composes pane-store's predicate queries with vector similarity search
- The MemX pipeline translates directly: pane-store attribute query (keyword equivalent) + vector similarity search, fused via RRF, re-ranked via four factors read from file attributes, filtered by rejection threshold
- Live queries for ongoing monitoring: "notify me when a memory matching these criteria appears"

### 5b. Memory consolidation as a kit-level protocol

The AI Kit should define a **memory consolidation protocol**: the process by which raw interaction (conversations, observations, task results) becomes structured memory.

Informed by MemX's granularity finding (fact-level doubles retrieval quality):

1. **Raw capture.** Conversation transcripts, observation logs, task results are written to `~agent/journal/` as timestamped files.
2. **Fact extraction.** The agent (or a consolidation routine) processes journal entries and extracts atomic facts. Each fact becomes a file in `~agent/memories/` with typed attributes.
3. **Relationship linking.** When a new memory contradicts, extends, or supersedes an existing one, the relationship is expressed as an xattr (`user.pane.supersedes=/path/to/old-memory`) and/or a symlink. Pane-store indexes these relationships.
4. **Embedding.** Each memory file gets an embedding vector computed by the agent's configured model (local or remote, governed by routing rules). Stored as `user.pane.embedding` xattr.
5. **Deduplication.** Before writing a new memory, query pane-store for similar content (vector similarity > threshold). If a near-duplicate exists, either skip or update the existing memory's attributes (bump importance, update timestamps).

This protocol is declarative -- it runs as a periodic job or on-event trigger, governed by the agent's `.plan` file. It's inspectable (every step produces files), auditable (version control on the memories directory), and transparent (the user can read any memory the same way they read any other file).

### 5c. Routing rules as memory governance

Make explicit what the architecture spec implies:

- **Sensitivity routing.** Memories extracted from conversations about `~/work/confidential/` inherit a `user.pane.sensitivity=confidential` attribute. Routing rules ensure these memories are never sent to remote APIs for re-embedding or any other purpose. Landlock enforces the boundary.
- **Sharing routing.** An agent can "publish" a memory to a shared knowledge directory by creating a hard link (same file, new location with broader permissions). Routing rules govern which memories are eligible for sharing based on attributes.
- **Retention routing.** Memories past a configured age with low importance and low retrieval count are candidates for archival. A routing rule moves them to cold storage or deletes them. The agent's `.plan` specifies retention policy.

### 5d. The xattr size constraint must be addressed

MemX stores 1024-dimensional float32 embeddings. That's 4,096 bytes per vector. Linux xattrs have a filesystem-dependent size limit -- ext4 caps at 4KB total per inode, which is too small. The architecture spec notes this ("BFS gap") and says pane targets btrfs. Btrfs has no hard xattr size limit (attributes up to 64KB with inline extents).

For embedding storage specifically, the recommendation is:

1. **On btrfs (pane's target):** Store embeddings directly as xattrs. 4KB is comfortably within btrfs's limits.
2. **Sidecar index as alternative.** If xattr storage proves impractical (performance, tooling), pane-store can maintain a sidecar vector index file (similar to MemX's approach) that maps file paths to embeddings. This is a pane-store implementation detail, not an API change -- the query interface is the same either way.
3. **Embedding dimensionality is a routing decision.** Local models produce smaller embeddings; remote models may produce larger ones. The routing rules that govern local vs. remote inference also determine embedding dimensionality. The AI Kit should normalize: if an agent switches models, re-embed on next consolidation cycle.

### 5e. Access/retrieval separation maps to pane attributes

Adopt MemX's insight directly:

- `user.pane.access_count` and `user.pane.last_accessed` -- updated when a memory file is explicitly read (agent or user opens it)
- `user.pane.retrieval_count` and `user.pane.last_retrieved` -- updated when a memory file is returned as a pane-store query result

The re-ranking formula uses retrieval metrics, not access metrics. This prevents an agent's routine scanning of its own memories from inflating their ranking. It's a small design decision with measurable impact on ranking quality (MemX demonstrates the ranking reversal).

Pane-store's fanotify integration can track access events (FAN_ACCESS) and update the access attributes. Retrieval attributes are updated by the query engine itself.

### 5f. What the AI Kit should NOT provide

- **Its own database.** Use the filesystem + pane-store.
- **Its own embedding storage.** Use xattrs + pane-store's vector index.
- **Its own query language.** Extend pane-store's BQuery-derived predicates with vector similarity operators.
- **Its own deduplication logic.** This is a pane-store feature (query for similar content before insert).
- **Its own retention/lifecycle management.** This is routing rules + cron (or agent `.plan` scheduled tasks).

The AI Kit provides: the memory consolidation protocol (journal to facts), the retrieval composition (predicate + vector + re-ranking + rejection), and the integration with the agent's `.plan` (what models to use, what to consolidate, what to share, what to retain). Everything else is existing infrastructure.

---

## Summary Table

| MemX Component | Pane Equivalent | Gap? |
|---|---|---|
| libSQL storage | Filesystem + pane-store attributes | None (pane's approach is better for the architecture) |
| Vector embeddings | `user.pane.embedding` xattr + pane-store vector index | **Yes: pane-store needs vector similarity search** |
| FTS5 keyword search | pane-store predicate queries over attributes | Partial (pane-store needs full-text search on file content, not just attributes) |
| RRF fusion | Composition in pane-ai retrieval function | None (straightforward to implement) |
| Four-factor re-ranking | Pane attribute reads + ranking function in pane-ai | None |
| Rejection rule | Default behavior of pane-ai memory retrieval | None |
| Tag-signature dedup | pane-store query for near-duplicates before write | None |
| memory_links graph | xattr relationships + pane-store graph queries | Partial (pane-store would need relationship-following queries) |
| Access/retrieval separation | Separate pane attributes for access vs. retrieval | None |
| Temporal indexing (future) | `user.pane.event_time` attribute + pane-store range queries | None (pane has this natively) |

**The one genuine capability gap is vector similarity search in pane-store.** Everything else that MemX provides is either already in pane's infrastructure or is a straightforward composition of existing primitives. This gap is worth closing -- it benefits not just agent memory but any content that benefits from semantic search (mail, documents, notes, code).

---

## Disposition

MemX is a well-engineered system that validates several design choices pane should adopt (hybrid retrieval, rejection rules, access/retrieval separation, fact-level granularity). But its architecture is fundamentally application-level -- it builds its own storage, its own indexing, its own query engine because it has no system-level infrastructure to lean on.

Pane has that infrastructure. The filesystem-as-database pattern, the attribute indexing, the live queries, the routing rules, the multi-user permission model -- these were designed (going all the way back to BFS) for exactly this class of problem. The right move for pane-ai is not to embed MemX or build something like it, but to extend pane-store with vector similarity search and implement the memory consolidation protocol as a kit-level concern on top of the existing infrastructure.

The result will be an agent memory system that is simultaneously more powerful (multi-agent, graded access, multi-view consistent, routable, transparent) and simpler (no separate database, no custom storage, no parallel query engine) than what MemX provides. That's the payoff of infrastructure-first design: you build the general machinery once, and specific applications compose it rather than reinventing it.
