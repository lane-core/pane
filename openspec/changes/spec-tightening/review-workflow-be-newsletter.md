# Workflow Spec Audit: Be Newsletter Lessons

Assessment of `openspec/specs/workflow/spec.md` against engineering practices documented in the Be Newsletter archive, with proposed additions.

---

## Per-Requirement Assessment

### Change-driven development
**Catches Be problem?** Yes — indirectly. Be shipped changes that seemed local but broke cross-component assumptions (the BMessage pointer constructor was "just a convenience" until it wasn't). The change proposal workflow forces articulation before implementation.

**Missing:** The workflow doesn't distinguish between changes that modify protocol types and changes that don't. At Be, any change to the app_server's message vocabulary affected every Interface Kit consumer. Pane's session types make this compile-visible, but the workflow should still flag protocol changes as requiring broader review than implementation changes.

### Pre-commit review
**Catches Be problem?** Yes. Hoffman's async batching insight (Newsletter #2-36) was discovered through performance profiling, not code review — but a reviewer trained to spot sync calls in hot paths would have caught it earlier. The review checklist doesn't currently include performance pattern review.

### Review checklist
**Catches Be problem?** Partially. Items 1-5 are solid. But the BMessage(BMessage*) debacle (Newsletter #4-46, Owen Smith) was a type-level mistake that passed all five checks: the code worked, it compiled, tests passed, references were accurate, the approach seemed valid. The problem was implicit conversion semantics — a category of bug that requires checking *what the types allow*, not just *what the code does*.

**Missing:** A check for unintended type coercions and implicit conversions, especially in public API surface.

### Zero-failure test policy
**Catches Be problem?** Not directly applicable — BeOS didn't have this discipline. But the policy is correct and Be would have benefited from it.

### Discovery-driven restart
**Catches Be problem?** Yes — this is exactly what Be should have done with the sync/async app_server distinction. The realization that sync calls were 20x slower than async should have triggered a protocol redesign, not a documentation note saying "avoid sync calls." The constraint was discovered mid-implementation and patched forward as guidance rather than being treated as spec.

### Session type consistency
**Catches Be problem?** Yes. This is the compiler doing what Be's commandments tried to do by convention. The dual-update requirement is exactly right.

### Message type discipline
**Catches Be problem?** Yes. BMessage's stringly-typed field access (`FindString("name")`) was a constant source of silent failures. Pane's typed enums with proptest coverage directly addresses this.

### Filesystem caching invariant
**Catches Be problem?** Yes. Cisler's thread synchronization articles (Newsletter #3-33) describe exactly this class of bug — threads making assumptions about state that's changed underneath them. The caching invariant is the right discipline.

---

## Proposed Additions

### 1. API Deprecation Protocol

The BMessage(BMessage*) story is instructive. Owen Smith (Newsletter #4-46): a pointer constructor that enabled implicit conversions and const-correctness violations. It shipped, applications used it, and deprecating it meant breaking those applications. "Between these issues, the const issue, and redundancy, we chose to deprecate this constructor instead."

Be's mistake wasn't the constructor — it was shipping without a deprecation protocol. By the time they recognized the problem, the API surface was in use.

**Proposed requirement: API stability protocol**

All public kit API changes SHALL follow a three-phase deprecation cycle:

1. **Introduce replacement** — new API ships alongside old. Old API annotated `#[deprecated(since = "version", note = "use X instead")]`.
2. **Warn period** — deprecated API produces compile warnings for one release cycle. Migration guide provided.
3. **Remove** — deprecated API removed. Removal is a change proposal, not a silent deletion.

Phase 1 to phase 3 SHALL span at least two release cycles.

API additions that expand trait bounds, add required trait methods, or change enum variants in non-`#[non_exhaustive]` types SHALL be treated as breaking changes and require a change proposal regardless of size.

**Scenario: Convenience method enables implicit conversion**
A kit method accepting `impl Into<PaneMessage>` is found to accept unintended types via blanket impls. The fix: deprecate the broad signature, introduce a specific one, remove after one release cycle.

**Scenario: Session type enum variant rename**
Renaming `TagUpdate` to `TagContent` in pane-proto is a breaking change affecting every consumer. It requires a change proposal even though it's "just a rename."

### 2. Protocol Evolution Protocol

When the compositor's session type definition changes, every consumer must update. The compiler catches shape mismatches, but the workflow should govern *how* changes propagate.

**Proposed requirement: Protocol change propagation**

Any change to a session type definition in pane-proto SHALL:

1. Be proposed as a change with explicit rationale for the protocol evolution.
2. Include a compatibility assessment: which crates consume the changed type? Which will fail to compile?
3. Update all consuming crates in the same commit (or change set). Partial protocol changes — updating the definition but not the consumers — SHALL NOT be committed, even if the unchanged consumers happen to compile (e.g., via wildcard patterns that silently absorb new variants).

**Hazard:** Wildcard match arms (`_ => {}`) silently absorb new protocol variants. A new variant that should trigger behavior gets swallowed. The review checklist (item 4, approach validity) should explicitly flag wildcard arms on session type enums.

**Scenario: New compositor event variant**
Adding a `ScaleChanged` variant to the compositor's event enum. Three kit-level consumers have `_ => { log::warn!(...) }` arms. The compiler doesn't catch it. The review must.

### 3. Performance Regression Detection

Schillings' benaphore work (Newsletter #1-26) demonstrated that Be engineers measured at the microsecond level. The difference between 35us (semaphore) and 1.5us (benaphore) was 20x and worth an optimization. Hoffman (Newsletter #2-36) showed that sync vs async app_server calls had measurable performance cliffs.

The workflow currently has no performance discipline. Build and test pass/fail is necessary but not sufficient — a change can pass all tests while doubling message latency.

**Proposed requirement: Performance-sensitive path annotation**

Code paths identified as performance-sensitive SHALL be annotated and benchmarked:

1. **Annotated paths:** Message dispatch (looper recv-to-handler), compositor frame assembly, session type send/recv, pane-store query evaluation, FUSE operation handling.
2. **Benchmark suite:** `cargo bench` targets for annotated paths using criterion. Benchmarks run as part of the review checklist (added as item 5b, after build-and-test).
3. **Regression threshold:** >10% regression on any annotated benchmark requires explanation. >25% requires a change proposal justification.

Benchmarks are not tests — they measure trends, not correctness. A benchmark regression is a signal for investigation, not an automatic block.

**Scenario: Channel implementation change doubles dispatch latency**
Switching from `std::sync::mpsc` to `crossbeam` channels for the looper. All tests pass. Benchmarks show 2x latency on uncontested recv. Investigation reveals crossbeam's allocation strategy doesn't optimize for the single-producer case that dominates pane's looper pattern. The change is revised.

### 4. Sync-in-Hot-Path Detection

Hoffman (Newsletter #2-36): "Synchronous calls are much slower than asynchronous calls, for several reasons. First, the Interface Kit caches asynchronous calls and sends them in large chunks at a time. A synchronous call requires that this cache be flushed."

This is the single most actionable performance insight from the Be Newsletter for pane's architecture. The workflow should encode it.

**Proposed requirement: Async-default enforcement**

The review checklist SHALL include a check for synchronous (blocking) calls in the following paths:

- Compositor main loop (calloop callbacks)
- Per-pane server threads during message dispatch
- Client-side looper threads during message handling
- Any code path that executes per-frame or per-input-event

A blocking call in these paths is a review finding of severity **moderate** unless justified. Justified exceptions (e.g., a one-time initialization sync call) SHALL be documented with `// SYNC: <reason>` comments.

**What counts as blocking:** `recv()` without timeout on a channel where the sender is in another process. `Mutex::lock()` on a lock held by a thread that does I/O. Any filesystem read (covered by the existing caching invariant, but this generalizes it). Any DNS resolution, network I/O, or database query.

**What doesn't count:** `Mutex::lock()` on a component's own state lock (uncontested fast path, per the benaphore lesson). Channel operations within a single process where the sender is a dedicated thread (bounded latency).

**Scenario: Routing rule evaluation blocks on filesystem read**
The pane-app kit evaluates routing rules by reading rule files from `/etc/pane/route/rules/` on each route action. Review catches this: rules should be cached in memory and updated via pane-notify, not re-read per dispatch. The existing caching invariant covers filesystem reads specifically, but this requirement generalizes the principle to any blocking operation.

### 5. Wildcard Match Arm Discipline

This deserves its own requirement because it's the specific escape hatch that defeats session type safety.

**Proposed requirement: No wildcard arms on protocol enums**

Pattern matches on session type enums and protocol message enums defined in pane-proto SHALL NOT use wildcard (`_`) or catch-all (`other`) arms. Every variant SHALL be matched explicitly.

**Rationale:** The entire point of typed protocol enums is exhaustive handling. A wildcard arm converts a compile-time protocol evolution check into a silent runtime swallow. This directly negates the advantage session types provide over BeOS's convention-based protocol discipline.

**Exception:** Logging/metrics code that genuinely needs to handle "any message" may use wildcards if it does not affect control flow (i.e., the wildcard arm logs and then the match continues to an exhaustive handling block).

**Scenario: Refactor adds variant, wildcard swallows it**
A developer adds `FocusStolen` to the pane event enum. The compositor's per-pane thread has `_ => {}` on events it doesn't handle yet. `FocusStolen` is silently ignored. The user sees no visual feedback when focus changes due to another pane's action. Without the wildcard, the compiler would force the developer to add an explicit empty handler, which would appear in review and prompt the question: "should this variant have behavior?"

### 6. Two-Failure Stop Rule (codify existing practice)

The workflow has "approach-level escalation" and "discovery-driven restart" but doesn't have an explicit circuit breaker for repeated implementation failure. The CLAUDE.md working contract has this rule — it should be in the workflow spec too, since the workflow governs agent behavior.

**Proposed requirement: Two-failure stop**

Two consecutive failures on the same implementation goal — where a failure is a review verdict of REVISE on the same issue, or a test failure caused by the same root cause — SHALL trigger a full stop. The agent SHALL:

1. State what it knows, what it doesn't, and what it has tried.
2. Not attempt a third fix.
3. Wait for direction or escalate to a design-level reassessment.

**Rationale:** Sunk cost pressure causes agents (and humans) to keep patching forward. The benaphore story is the positive version of this: Schillings didn't keep trying to make semaphores faster — he recognized the approach was wrong and designed a new primitive. The workflow should force the same discipline.

---

## What the Workflow Gets Right

To be clear about what doesn't need changing:

- **Contract-first implementation** is exactly the discipline Be lacked. BeOS conventions were right but unenforced. This requirement makes the architecture load-bearing.
- **Session type consistency** with compiler enforcement is the direct answer to Be's Commandment #1 being convention-only.
- **Discovery-driven restart** captures the key lesson from Be's sync/async realization.
- **Zero-failure test policy** with burden-on-agent is the right default for a system where compositor crashes take down the display.
- **TODO.md capture** prevents the "fix it while I'm here" impulse that introduces unrelated regressions.

## Summary of Proposed Changes

| Addition | What it catches | Be Newsletter source |
|---|---|---|
| API deprecation protocol | Implicit conversion bugs, breaking changes without migration path | Owen Smith, #4-46 (BMessage pointer constructor) |
| Protocol evolution protocol | Silent variant absorption via wildcards, partial protocol updates | Hoffman, #2-36 (app_server message vocabulary) |
| Performance regression detection | Latency regressions invisible to correctness tests | Schillings, #1-26 (benaphore: 35us vs 1.5us) |
| Sync-in-hot-path detection | Blocking calls that destroy responsiveness | Hoffman, #2-36 (sync calls flush cache, force round-trip) |
| Wildcard match arm discipline | Defeat of exhaustive protocol checking | Structural consequence of typed enums replacing BMessage `what` codes |
| Two-failure stop rule | Sunk cost patching forward | Inverse of Schillings' benaphore (recognized wrong approach, designed new one) |
