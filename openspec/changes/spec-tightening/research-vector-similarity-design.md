# Vector Similarity Search in pane-store: Design Analysis

Research for pane spec-tightening. Primary sources: Haiku BFS/BQuery implementation (`~/src/haiku/src/add-ons/kernel/file_systems/bfs/`, `~/src/haiku/headers/private/file_systems/QueryParser.h`), MemX analysis (`research-memx-local-memory.md`), architecture spec (`openspec/specs/architecture/spec.md`), Giampaolo's BFS design (via "Practical File System Design" and Be Newsletter archives).

---

## 1. Where Vector Similarity Lives

### The options and why most are wrong

**Option C (in the kit / client-side) is wrong.** The client doesn't have the index. Computing similarity from cached embeddings means every client maintains its own copy of every embedding it might need to compare against. This defeats the BFS principle: the *system* knows the data, and applications query the system. If you push similarity into the kit, you've built a database in every application -- exactly the antipattern BFS was designed to eliminate.

BFS's lesson was specific: when email clients each maintained their own header databases, you got siloed data, inconsistent indices, and no cross-application search. When BFS moved the index into the filesystem, every application got search for free. Pushing vector search into clients would recreate exactly this fragmentation.

**Option D (FUSE overlay) is wrong.** Filesystem-level vector indexing is a category error. FUSE operates at the VFS layer -- open, read, write, getxattr. It has no concept of "find me the k nearest neighbors in embedding space." You'd have to encode the entire query protocol into ioctl or xattr conventions, which is grotesque. The filesystem layer provides storage and metadata; the indexing service provides queries. BFS understood this separation -- the B+ tree index was a filesystem feature, but `BQuery` was a separate API surface that *used* the index. Don't conflate the storage layer with the query layer.

**Option B (separate pane-vector service) is architecturally wrong for pane.** This is the microservice instinct, and it's wrong here. The BLooper model says each server has *self-contained operational semantics* -- a server owns a domain and handles it completely. pane-store's domain is "attribute indexing and queries over files." Vector similarity over file embeddings is a query over file attributes. Splitting it into a separate service means:

- Two services need to be coordinated for a single query (predicate + vector)
- Live query notifications need to flow through both services
- The fusion step (combining predicate results with vector results) lives... where? A third service? The client? Neither is good
- You've broken the single-responsibility of pane-store without gaining anything. What does pane-vector know that pane-store doesn't? They index the same files

The only argument for separation is resource isolation -- vector indexing is CPU/memory intensive, and you might not want it competing with predicate queries. But pane already uses per-component threading. pane-store can run its HNSW operations on dedicated threads without needing a process boundary.

**Option A (in pane-store itself) is right.** The vector index is another index, like the attribute B+ tree. It lives alongside the attribute index, is updated by the same fanotify event loop, participates in the same query evaluation, and emits notifications through the same live query mechanism. This is the BFS principle applied directly: the indexing service indexes. It doesn't matter whether the index is a B+ tree over string values or an HNSW graph over float vectors -- it's an index over file metadata, and pane-store is the file metadata indexing service.

### What this looks like concretely

pane-store maintains two index structures:

1. **Attribute index.** In-memory (rebuilt on startup from xattr scan). Maps attribute names to sorted value sets. Supports predicate evaluation: equality, comparison, range, prefix/suffix matching. This is the BFS B+ tree equivalent.

2. **Vector index.** HNSW graph over embedding vectors. Maps file paths to embedding vectors and supports approximate k-NN queries. Persisted to disk (HNSW construction is expensive; you don't want to rebuild from xattr scan on every restart). Loaded into memory at startup.

Both indices are updated from the same event source: fanotify notifications when `user.pane.*` xattrs change. When pane-store sees a `user.pane.embedding` xattr written or modified, it reads the vector and updates the HNSW index. When it sees any other `user.pane.*` xattr change, it updates the attribute index. Same event loop, same file watcher, two index targets.

### The Rust implementation surface

The `instant-distance` crate provides a pure-Rust HNSW implementation with `serde` support (for persistence) and SIMD-accelerated distance computation. It's small, no-std compatible, and well-maintained. For a system that controls its own dependencies completely, this is the right choice -- no C++ bindings, no FFI, no hnswlib wrapping.

Alternative: `hnsw_rs` (Rust bindings to hnswlib). More mature index, but introduces C++ dependency. Given pane's preference for controlling its dependency chain, `instant-distance` or a similar pure-Rust implementation is better.

The embedding vector itself is stored as a `user.pane.embedding` xattr: a binary blob containing a header (dimensionality as u16, model hash as u64) followed by the float32 vector. On btrfs, a 1024-dim float32 vector is 4,106 bytes (10 bytes header + 4,096 bytes data) -- well within btrfs's ~16KB xattr limit.

---

## 2. Query Interface: Predicates Meet Vectors

### The fundamental tension

BQuery's query language is boolean. A file either matches `MAIL:status == "New" && BEOS:TYPE == "text/x-email"` or it doesn't. The result set is unordered (BFS returns results in B+ tree traversal order, which is index key order, not relevance order).

Vector similarity is ranked. "Find me the 10 files most similar to this embedding" returns an ordered list with scores. There's no boolean "match" -- every file with an embedding has *some* similarity to the query vector. The question is where you draw the cutoff.

These are genuinely different operations. Trying to shoehorn vector similarity into BQuery's predicate syntax would be a mistake -- it would look like `user.pane.embedding ~= <vector> WITH k=10 AND threshold=0.5`, which is syntactically possible but semantically incoherent. A predicate evaluates to true/false per file. A vector query evaluates to a ranked list across all files. They compose, but they are not the same thing.

### The design: two query paths, one fusion point

BQuery in Haiku works by building an expression tree of `Term` nodes (either `Equation` leaf nodes or `Operator` internal nodes for AND/OR). Each `Term` has a `Match()` method that evaluates a single file against the predicate, returning MATCH_OK or NO_MATCH. For initial fetch, the query engine walks the B+ tree index to find candidate files efficiently, then evaluates the full predicate against each candidate.

pane-store's query interface should work similarly, with an extension:

**Predicate query** (BQuery-equivalent):
```
type == "memory" && tags contains "debugging" && created > "2026-03-01"
```
Returns: unordered set of matching file paths.

**Vector query**:
```
similar_to(<embedding_vector>, k=20, threshold=0.4)
```
Returns: ordered list of (file_path, similarity_score) pairs, at most k results, all above threshold.

**Hybrid query** (the interesting case):
```
{
  predicate: "type == 'memory' && tags contains 'debugging'",
  vector: { embedding: <vector>, k: 20, threshold: 0.4 },
  fusion: "rrf"  // or "predicate_first", or "vector_first"
}
```
Returns: ordered list of (file_path, score) pairs.

### Fusion strategies

Three fusion modes, selectable by the caller:

1. **`predicate_first`** (filter-then-rank). Execute the predicate query to get a candidate set, then rank candidates by vector similarity. Fast when the predicate is selective. This is the common case for agent memory: "find debugging memories similar to this query" first narrows to debugging memories, then ranks by embedding similarity. The vector index only needs to compute distances for the filtered set, not the entire corpus.

2. **`vector_first`** (rank-then-filter). Execute the vector query to get top-k candidates, then filter by predicate. Fast when k is small and the predicate is expensive. Useful for "find the 5 files most similar to this, but only if they're text files."

3. **`rrf`** (reciprocal rank fusion). Execute both queries independently, combine rankings via RRF. This is the MemX-validated approach: it handles the case where predicate and vector signals disagree. A file might rank low on vector similarity but match the predicate exactly (high keyword relevance), or rank high on vector similarity but not match the predicate (the semantic is right but the metadata is missing). RRF captures both signals.

The default should be `predicate_first` when a predicate is present, `vector_first` when only a vector query is given. `rrf` is opt-in for cases where the caller knows they want both signals independently weighted.

### The API surface for kit developers

From the kit developer's perspective, this is one query API with optional components:

```rust
// Pure predicate query (BQuery equivalent)
let results = store.query()
    .predicate("type == 'memory' && tags contains 'debugging'")
    .fetch()?;

// Pure vector query
let results = store.query()
    .similar_to(&embedding, k: 20, threshold: 0.4)
    .fetch()?;

// Hybrid query, predicate-first fusion
let results = store.query()
    .predicate("type == 'memory' && tags contains 'debugging'")
    .similar_to(&embedding, k: 20, threshold: 0.4)
    .fetch()?;

// Hybrid query, explicit RRF fusion
let results = store.query()
    .predicate("type == 'memory' && tags contains 'debugging'")
    .similar_to(&embedding, k: 20, threshold: 0.4)
    .fusion(Fusion::Rrf { k: 60 })
    .fetch()?;
```

The builder pattern is deliberate. BQuery used setter methods (`SetVolume()`, `SetPredicate()`, `SetTarget()`) followed by `Fetch()`. The builder is the same pattern with Rust idiom. The critical difference: BQuery had no ranking concept. pane-store's query results carry a `score` field that is 1.0 for pure predicate matches (boolean -- you matched) and a float in [0,1] for vector and hybrid queries.

### What BQuery got right that we preserve

1. **The predicate language is string-based.** BQuery accepted predicate strings (`SetPredicate("MAIL:status == New")`) as well as programmatic construction (RPN push). Pane-store should accept predicate strings for the same reason: they're scriptable. `pane-store query 'type == "memory" && importance > 0.7'` from the shell is the equivalent of `query "MAIL:status == New"` on BeOS. The scripting protocol needs string predicates.

2. **At least one indexed attribute required.** BQuery required that the predicate reference at least one indexed attribute, to ensure the engine could use an index rather than scanning every file. pane-store should have the same requirement: a predicate-only query must reference at least one indexed attribute. A vector-only query uses the vector index. A hybrid query has at least one index path.

3. **Volume scoping.** BQuery targeted a specific BFS volume. pane-store indexes a specific filesystem mount. The scope is the same.

---

## 3. What BeOS Problems Vector Similarity Clarifies

### Real wins

**The Tracker search experience.** This is the biggest one, and it would have changed BeOS's trajectory if we'd had it. The Find panel required knowing attribute names and having a guess at values. "Find emails from Rob about the deadline" required translating to `MAIL:from == "*rob*" && MAIL:subject == "*deadline*"`. Users who didn't think in predicates couldn't use Find effectively. With vector similarity, the query is: embed the natural language string "emails from Rob about the deadline" and do a hybrid search with `type == "text/x-email"` as the predicate component. The vector handles the semantic matching; the predicate handles the structural filtering. This would have been transformative for Tracker.

Giampaolo's own observation (from Be Newsletter Issue 3-4, discussing the Find panel's case-insensitive queries) was that the system needed to compensate for users not remembering exact details. Case-insensitivity was the 1998 version of this. Semantic similarity is the 2026 version. Same principle: the system should find what you mean, not just what you typed.

**Mail organization.** BeOS mail was the crown jewel of the BFS-as-database pattern. Every header was an attribute, every query was a virtual folder. But the pattern broke down for content-based search. "Find that email where someone mentioned the budget numbers" required knowing which attribute to query and what value to look for. The mail *body* was file content, not an indexed attribute. BFS could search attributes but not file content efficiently.

With embeddings, the mail body gets an embedding stored as `user.pane.embedding` on the file. Now "find emails about budget numbers" is a vector query over mail file embeddings, filtered by `type == "text/x-email"`. The attribute system handles the structural metadata (from, to, date, status) and the vector index handles the semantic content. This is the completion of the BFS mail pattern that we never got to build.

**Contact/person resolution.** "Find communications from the person who was working on the renderer" is a multi-hop query that crosses content types. With predicate-only search, you'd need to know the person's name, find their email address, then search for emails from that address. With vector similarity, you embed the query and search across all content types -- emails, documents, chat logs, commit messages -- because the embedding captures the semantic relationship "person working on the renderer" regardless of which attribute or content type encodes it.

### Moderate wins

**Application matching.** The roster matched apps to content via exact MIME type strings. This worked for common types and broke for the long tail. A `.numbers` file without a MIME type couldn't be matched to a spreadsheet application. Semantic matching could help here: embed the file content, compare against embeddings of application descriptions, find "this file is semantically similar to things that spreadsheet applications handle." But this is a moderate win because the MIME system, when it worked, was simpler and more predictable. Exact type matching is a feature, not a bug -- you want deterministic application dispatch, not probabilistic guessing. The right use for vector similarity here is as a *fallback*: when the MIME system can't identify a file, semantic matching provides a best guess. Not a replacement for the type system.

**File type identification.** Same analysis as application matching. The MIME sniffer (I can see it in Haiku at `src/kits/storage/mime/SnifferRules.cpp` -- priority-ordered rules with byte-pattern matching) is *correct* when it works. It's deterministic, fast, and produces exact types. Embeddings would help for the failure case -- when the sniffer doesn't match any rule, and the file has no `BEOS:TYPE` attribute. But embedding-based type identification is slow (requires inference) and approximate. It's a supplementary signal, not a replacement.

### Theoretical wins (not worth building for)

**Cross-type semantic clustering.** "Show me everything related to Project X across all file types." This sounds useful in the abstract but doesn't match how people actually work. Users navigate by location (directory), by type (query on MIME type), or by time (recent files). Semantic clustering is a feature nobody asked for because the existing navigation patterns are sufficient. If a user wants to find Project X files, they look in the Project X directory. The filesystem hierarchy already provides this grouping. Don't build infrastructure to solve problems that directory structure already solves.

### What the Be engineers would have built

If we'd had embeddings in 1998, I believe Dominic would have added it as a fourth index type alongside name, size, and last_modified. Not as a separate system -- as another index that the query engine understood natively. The query language would have gained a `~=` operator or similar, and the Find panel would have gained a "similar to" search mode.

We would *not* have replaced the MIME system or the sniffer. We would not have replaced predicate queries. We would have added vector search as another tool in the same toolbox, accessible through the same query infrastructure, producing results in the same format. That's the BFS design philosophy: uniform infrastructure, not parallel systems.

And honestly, we would have shipped it in a newsletter article and a sample app before it was fully baked, because that's how we rolled. The infrastructure would have been right; the UI integration would have taken another release.

---

## 4. Live Queries with Vector Similarity

### How BFS live queries work (from the source)

The mechanism is in `QueryParser.h`, specifically `_EvaluateLiveUpdate()` (line 1639 in Haiku's implementation). It's elegant in its simplicity:

```cpp
// When an attribute changes on a file:
status_t oldStatus = fExpression->Root()->Match(entry, node, attribute,
    type, oldKey, oldLength);      // did the file match BEFORE the change?
status_t newStatus = fExpression->Root()->Match(entry, node, attribute,
    type, newKey, newLength);      // does it match AFTER?

if (oldStatus != MATCH_OK && newStatus == MATCH_OK)
    opcode = B_ENTRY_CREATED;      // file entered the result set
else if (oldStatus == MATCH_OK && newStatus != MATCH_OK)
    opcode = B_ENTRY_REMOVED;      // file left the result set
```

The key insight: live query evaluation is *differential*. It doesn't re-run the query. It takes the old and new values of the changed attribute, evaluates the predicate twice (once with old value, once with new), and compares the results. If the file's match status changed, it emits a notification. This is O(1) per attribute change per live query -- not O(n) over the corpus.

This is called synchronously from `Index::Update()` (line 245 in `Index.cpp`): when an attribute changes and the index is updated, `fVolume->UpdateLiveQueries()` iterates all registered live queries and calls `LiveUpdate()` on each. The attribute write doesn't return until all live queries have been evaluated. This is the strong consistency guarantee: you never observe a state where the attribute has changed but the live query hasn't been notified.

### The problem with vector similarity live queries

Predicate live queries work because predicate evaluation is a function of one file's attributes. "Does file X match `type == 'memory' && importance > 0.7`?" depends only on file X's attributes. When file X's attributes change, you re-evaluate the predicate for file X. No other files need to be considered.

Vector similarity is not like this. "Is file X in the top-k most similar to query vector Q?" depends on *every other file's embedding*. When a new file Y is added with an embedding, file X might be pushed out of the top-k even though X's embedding didn't change. When file Z's embedding is updated, the entire ranking might shift.

This is the fundamental problem: predicate live queries are *local* (each file's membership is determined by its own attributes) while vector live queries are *global* (each file's membership is determined by its relationship to all other files).

### The design: two notification modes

**Predicate-only live queries work exactly as BFS.** When a `user.pane.*` attribute changes, pane-store evaluates registered predicate live queries differentially, emits ENTRY_CREATED/ENTRY_REMOVED notifications. Same semantics as BFS. No change needed.

**Vector live queries use a threshold-based model, not top-k.** This is the key design decision. A live query with a vector component is registered with a *similarity threshold*, not a k value:

```rust
let live_query = store.query()
    .predicate("type == 'memory'")
    .similar_to(&embedding, threshold: 0.6)  // no k -- threshold only
    .live(my_handler)
    .fetch()?;
```

With a threshold, live query evaluation becomes local again. When a new file is added with an embedding, pane-store computes its similarity to the query vector. If similarity >= threshold, emit ENTRY_CREATED. When a file's embedding changes, compute new similarity. If it crossed the threshold in either direction, emit the appropriate notification. This is O(1) per embedding change per live query -- the same complexity as predicate live queries.

Why not top-k for live queries? Because top-k live queries require maintaining a global ranking. Every embedding change potentially reshuffles the ranking, requiring re-evaluation of all files in the index. For a corpus of 100k files and 50 active live queries, every embedding write triggers 50 * 100k distance computations. That's not viable, and it would destroy the synchronous notification guarantee that makes live queries useful.

The threshold model trades precision for tractability. A top-k query says "give me exactly 20 results." A threshold query says "give me everything above 0.6 similarity." The threshold query might return 5 results or 50 -- it's not bounded. But it's locally evaluable, which means it composes with pane-store's event-driven architecture.

For one-shot queries, top-k is still available (the `k` parameter on `similar_to()`). It's only live queries that require the threshold formulation.

**Hybrid live queries combine both modes.** A live query with both predicate and vector components:

1. When an attribute changes on a file: evaluate the predicate differentially (old/new). If the file's predicate status changed, also compute vector similarity against the threshold. Emit ENTRY_CREATED if the file now matches the predicate AND exceeds the similarity threshold. Emit ENTRY_REMOVED if either condition is no longer met.

2. When an embedding changes on a file: compute new similarity against the threshold. If the file matches the predicate AND the similarity crossed the threshold, emit the appropriate notification.

This is still O(1) per change per live query. The predicate and vector components are evaluated independently and AND-composed for the notification decision.

### Notification message format

BQuery's live update messages carry `opcode`, `name`, `directory`, `device`, `node`. pane-store's should carry the same fields (translated to pane's protocol) plus:

- `similarity_score`: float, present when the query has a vector component. For ENTRY_CREATED, this is the current similarity. For ENTRY_REMOVED, this is the similarity that triggered removal (either below threshold, or the file lost its embedding).

This lets the client maintain a sorted view of live query results without re-querying.

---

## 5. The Embedding Lifecycle

### Who computes embeddings

The agent or application that owns the file computes the embedding. pane-store does *not* compute embeddings -- it indexes them. This follows the BFS pattern: BFS didn't compute attribute values, it indexed them. Applications wrote attributes; BFS indexed them. Applications write embeddings as xattrs; pane-store indexes them.

The AI Kit provides the embedding computation infrastructure:
- Local model management (loading, inference scheduling)
- Embedding computation as a kit function: `ai_kit.embed(content) -> Vec<f32>`
- The routing layer determines which model handles embedding (local vs. remote, per the agent's `.plan`)

A non-AI-Kit application can also write embeddings directly as `user.pane.embedding` xattrs. The xattr is just bytes; pane-store doesn't care who wrote them.

### When embeddings are computed

Three triggers:

1. **On file creation/modification.** When an agent writes a memory file or an application saves a document, the embedding is computed and written as part of the save operation. This is the hot path -- the file is already in memory, the content is available, compute the embedding while you're at it.

2. **On consolidation.** The memory consolidation protocol (journal -> facts) computes embeddings for each extracted fact as part of the consolidation step. Batch processing -- consolidation runs periodically, not on every interaction.

3. **On re-index.** When the model changes or an admin requests recomputation. This is the cold path -- crawl the filesystem, read files without embeddings or with stale embeddings, compute and write.

### The model change problem

When the embedding model changes, old embeddings are incomparable with new ones. A 1024-dim vector from model A and a 1024-dim vector from model B might use entirely different feature spaces. Mixing them in the same HNSW index produces garbage results.

BFS faced an analogous problem: attribute type changes. If you had an index on `MAIL:priority` as B_STRING_TYPE and then an application started writing it as B_INT32_TYPE, the B+ tree index would contain mixed types. BFS handled this poorly -- the index just contained whatever was written, and queries against the wrong type would silently miss entries. Haiku's `Equation::Match()` (QueryParser.h line ~717) checks the attribute type during evaluation, but the index itself is type-unaware.

pane-store should handle this better. The embedding xattr includes a model hash in its header:

```
[u16: dimensionality][u64: model_hash][f32 * dimensionality: vector]
```

**Invariant: the HNSW index contains only embeddings from one model.** The model hash is recorded in the index metadata. When pane-store indexes a new embedding:

- If the model hash matches the index: insert normally.
- If the model hash differs: mark the file as "stale embedding" (add it to a stale set, do not insert into the HNSW index).

**Stale embeddings are invisible to vector queries but visible to predicate queries.** A file with a stale embedding still has all its other attributes indexed. It still shows up in predicate queries. It just doesn't participate in vector similarity search until its embedding is recomputed with the current model.

**Re-embedding is a background task.** When the model changes:

1. pane-store detects the first new-model embedding written (model hash differs from current index model hash).
2. It rebuilds the HNSW index from scratch with only new-model embeddings.
3. It publishes a notification: "model changed, N files have stale embeddings."
4. Agents and applications re-embed their files at their own pace. As each new embedding is written, pane-store indexes it into the new HNSW index.
5. The stale set shrinks over time. pane-store can report progress: "4,200 of 12,000 embeddings refreshed."

This is gradual migration, not atomic cutover. The system degrades gracefully: vector search works immediately for new-model files and gradually improves as old files are re-embedded. This is analogous to BFS's index creation behavior -- creating a new index only indexed files written after creation, not existing files. Existing files were indexed as they were touched.

### Files without embeddings

Not every file needs an embedding. Binary executables, configuration files, empty files -- these don't benefit from semantic search. The HNSW index only contains files that have a `user.pane.embedding` xattr. Files without embeddings are invisible to vector queries and visible to predicate queries.

This is the same as BFS: files without a particular indexed attribute were invisible to queries on that attribute. If you queried `MAIL:from == "rob"` and a file didn't have a `MAIL:from` attribute, it simply didn't match. No error, no special handling -- the file just wasn't in the index.

### Dimensionality changes

If the new model produces 768-dim vectors instead of 1024-dim, the entire HNSW index must be rebuilt. You cannot mix dimensionalities in the same index -- the distance function requires equal-length vectors.

The mitigation: the embedding xattr header includes dimensionality. pane-store checks dimensionality on every insertion. If it doesn't match the current index dimensionality, the embedding is marked stale. The re-embedding protocol handles it the same way as model hash changes -- gradual migration.

In practice, dimensionality changes should be rare. The AI Kit should standardize on a dimensionality and normalize to it. If a local model produces 384-dim vectors and a remote model produces 1536-dim, the kit truncates or pads to the standard dimensionality. This is a lossy operation, but it preserves index coherence. The alternative -- maintaining multiple HNSW indices at different dimensionalities -- is complexity that doesn't pay for itself.

**Recommendation: standardize on 1024 dimensions.** This is Qwen3-0.6B's native dimensionality, which the MemX paper validates as effective at the local scale. Larger models' embeddings are truncated via Matryoshka Representation Learning (most modern embedding models support this). Smaller models' embeddings are zero-padded (less ideal, but maintains index coherence).

---

## 6. Design Decisions Summary

| Decision | Choice | Rationale |
|---|---|---|
| Where vector similarity lives | In pane-store, alongside attribute index | BFS principle: one indexing service, one query interface. BLooper self-contained semantics. |
| Index implementation | HNSW, pure Rust (`instant-distance` or similar) | Pane controls its dependencies. No C++ FFI. Persisted to disk, loaded at startup. |
| Query interface | Builder pattern with `.predicate()` and `.similar_to()` | BQuery's setter pattern + Rust idiom. String predicates for scriptability. |
| Fusion strategy | `predicate_first` default, `rrf` opt-in | predicate_first is the common case (filter then rank). RRF for cases where signals disagree. |
| Live query semantics for vectors | Threshold-based, not top-k | Threshold is locally evaluable (O(1) per change per query). Top-k requires global ranking (O(n)). |
| Embedding storage | `user.pane.embedding` xattr with header (dim + model hash) | Filesystem-native. Visible to `getfattr`. Indexed by same fanotify event loop. |
| Model migration | Gradual. New index on first new-model embedding. Stale set shrinks as files re-embed. | Analogous to BFS index creation. Graceful degradation over atomic cutover. |
| Standard dimensionality | 1024 | Qwen3-0.6B native. Matryoshka truncation for larger models. Index coherence. |
| Who computes embeddings | Applications/agents via AI Kit. pane-store indexes, never computes. | BFS pattern: apps write attributes, filesystem indexes them. |

### What this means for the spec

The architecture spec already says the right thing about pane-store having vector similarity. This analysis fills in *how*:

1. **pane-store section** needs: the two-index architecture (attribute + HNSW), the hybrid query interface, the fusion strategies, the embedding xattr format.

2. **pane-store section** needs: the live query extension -- threshold-based vector live queries with O(1) notification semantics.

3. **pane-ai section** needs: the embedding lifecycle -- who computes, when, model migration protocol, dimensionality standardization.

4. **The query language spec** (when it exists) needs: the predicate grammar (carry forward from BQuery) plus the `similar_to()` clause.

5. **The scripting protocol** needs: shell access to hybrid queries. `pane-store query --predicate 'type == "memory"' --similar-to <embedding_file> --threshold 0.5` should work from the command line, same as `query "MAIL:status == New"` worked on BeOS.

### What we are not building

- A separate vector database service. pane-store is the database.
- An embedding computation service in pane-store. pane-store indexes; it doesn't infer.
- Top-k live queries. Too expensive. Threshold live queries provide the reactive semantics without the global ranking cost.
- Multiple HNSW indices for different dimensionalities. Standardize and normalize.
- Vector similarity in the FUSE layer. The filesystem provides storage; the query service provides queries.
