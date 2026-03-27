# Workflow: Development Protocols

Process invariants for how changes to pane are proposed, implemented, verified, reviewed, and committed. These are as load-bearing as behavioral contracts — violating them has caused real regressions and lost work in analogous projects.

## Purpose

Development, testing, review, and commit protocols that govern how changes to pane are validated and integrated. These exist because pane is a compositor (crashes take down the display), a protocol system (bugs propagate across process boundaries), and a distro foundation (regressions compound).

These protocols are designed for agent execution. Pane is built by its own inhabitants (see development methodology) — agents working within the system, following its protocols, exercising its infrastructure. The review and commit protocols encode the discipline that makes this safe.

## Requirements

### Requirement: Change-driven development

All non-trivial work SHALL go through the openspec change workflow: propose → contracts → architecture → tasks → apply → archive. Trivial fixes (typos, single-line bug fixes) may bypass the workflow but MUST still follow the review protocol.

The custom `pane` schema SHALL be used for all changes (`openspec new change --schema pane "<name>"`). Contracts and architecture are parallel — both depend on the proposal but not on each other.

#### Scenario: New server requires change proposal
Adding pane-notify as a crate requires a change proposal, contracts specifying its behavioral requirements, an architecture document with design decisions, and a task list.

#### Scenario: Typo fix bypasses workflow
Fixing a doc comment typo is committed directly with review but without a full change proposal.


### Requirement: Pre-commit review

Every commit SHALL require a correctness review before staging.

**Mechanism:**
- Non-trivial changes (multi-file, new types, approach-level, new crate): spawn a `feature-dev:code-reviewer` agent against the full diff.
- Small changes (single-file fixes, doc updates): the committing agent runs through the checklist inline.

The threshold is judgment-based: if you'd want a second pair of eyes on it in a human team, use the agent.

#### Scenario: New session type triggers agent review
Adding a new protocol branch to the pane-comp session type spawns a `feature-dev:code-reviewer` agent before any `git add`.


### Requirement: Review checklist

The review SHALL verify five items in order:

1. **Task completion** — diff accomplishes what was requested. Flag claimed-but-missing or present-but-unrequested changes.
2. **Contract compliance** — cross-reference against `openspec/specs/`, architecture spec, CLAUDE.md. Run `openspec validate --all`.
3. **Reference accuracy** — every type name, module path, crate reference, or cross-reference in the diff MUST be verified against current codebase state.
4. **Approach validity** — for non-trivial changes: is this the right approach? Does it respect session type contracts? Could it fail for reasons not visible in test results?
5. **Build and test** — `cargo build && cargo test` MUST pass. New warnings MUST be acknowledged or fixed. Test count MUST not regress.

#### Scenario: Stale module path caught
A doc comment citing `pane_proto::message::PlumbMessage` is flagged because that type no longer exists — it belongs to a removed inter-server routing model.


### Requirement: Review verdict

The review SHALL produce one of three verdicts:

- **PASS** — all checks satisfied, proceed to commit.
- **PASS with notes** — minor issues that don't block. Notes go in commit message or TODO.md.
- **REVISE** — issues listed with severity (critical/moderate/minor) and concrete fixes. MUST NOT commit until critical and moderate issues are resolved.

#### Scenario: Session type violation blocks commit
A review finding that a protocol branch accepts a message variant after a session endpoint has been consumed produces REVISE with severity critical.


### Requirement: Zero-failure test policy

Any test failure SHALL be treated as a regression until proven otherwise. The agent SHALL assume it caused any failure observed during its work. The burden of proof is on the agent to demonstrate a failure is pre-existing (by testing the pre-change state).

#### Scenario: Proptest failure during protocol work
A proptest failure in session type roundtrip tests during protocol changes is assumed to be caused by those changes. The agent tests the pre-change state before claiming pre-existing.


### Requirement: Discovery-driven restart

When a previously-unknown invariant or contract is discovered mid-implementation, it SHALL be treated as new spec, not as a bug in the current code. The agent SHALL re-evaluate design choices against the expanded specification before patching forward.

If the new invariant would have changed how the code was structured, restart from the expanded spec rather than retrofitting.

**Hazard**: Patching forward after discovering a missed invariant produces brittle code that technically works but structurally ignores the constraint.

#### Scenario: Session type constraint discovered
Discovering that a protocol branch requires the server to respond before the client can send the next message — a constraint not captured in the original session type definition — triggers a redesign of the session type, not a workaround in the handler.


### Requirement: Approach-level escalation

The agent SHALL stop and escalate when a review reveals the *approach itself* may be wrong — not just the implementation. Re-evaluate the design against expanded understanding rather than patching forward.

A wrong approach that passes tests is worse than a right approach with a failing test.

#### Scenario: Review questions the renderer architecture
During pane-comp implementation, a reviewer identifies that the glyph atlas approach can't handle emoji clusters correctly. The agent stops, escalates, and the rendering approach is reconsidered.


### Requirement: TODO.md capture protocol

When an agent notices something that should be fixed but isn't part of the current task, it SHALL add it to TODO.md with a brief description, severity, and enough context for someone to pick it up later. The agent SHALL NOT fix it inline — capture and move on.

#### Scenario: Agent notices missing validation during renderer work
While implementing the text renderer, the agent notices that a glyph region doesn't validate width > 0. It adds a TODO.md entry and continues with the renderer.


### Requirement: Contract-first implementation

When implementing a task, the agent SHALL read the relevant contracts BEFORE writing code. Contracts define WHAT the system SHALL do. The agent implements THAT, not what seems reasonable without checking.

The architecture is the prompt. Session types constrain what conversations are valid. Optics constrain how state is accessed. The kit APIs constrain what operations are available. An agent working within these constraints has less room to make mistakes — not because it's smarter, but because the design space has been narrowed to the region where correct implementations live (see development methodology).

**Hazard**: Writing code that "seems right" without checking contracts leads to implementations that pass tests but violate invariants not covered by tests.

#### Scenario: Session type implementation
Before adding a new protocol branch in pane-comp, the agent reads the session type definition in pane-proto to verify which branches exist, what message types are expected, and what the continuation type is after each exchange.


### Requirement: Filesystem caching invariant enforcement

Any code that reads configuration or filesystem state in a server's event loop SHALL be reviewed for caching invariant compliance. Servers cache filesystem state in memory at startup and update only in response to pane-notify events. The render loop and event dispatch SHALL NOT perform filesystem I/O.

**Hazard**: A single `fs::read()` in the render loop at 60fps produces 60 syscalls/second per config key.

#### Scenario: Config read in render path caught
A review catches `std::fs::read_to_string("/etc/pane/comp/font")` inside the frame rendering function. The fix: read on startup and on pane-notify event, cache in memory.


### Requirement: Session type consistency

Protocol types in pane-proto are the single source of truth for every protocol in the system (architecture spec §7). When a new message variant is added to a session type enum, all endpoints implementing the complementary (dual) session type MUST be updated. When a protocol's session structure changes (new branches, reordered exchanges), all consumers MUST be reviewed for compatibility.

The compiler enforces this via exhaustive pattern matching and session type duality — a change to one side that isn't reflected in the other fails to compile. The review checklist verifies that the intent behind the change (not just the compilation result) is correct.

#### Scenario: New protocol branch requires dual update
Adding a `Resize` branch to the compositor's pane session type requires the pane-app kit's client-side session handling to add the corresponding dual branch. The compiler catches this; the review verifies the semantics are correct.


### Requirement: README accuracy after major changes

After each major change to the codebase — new crates, removed crates, changed build sequence, new technology choices, milestone completions — the agent or developer SHALL assess whether README.md needs updating. The README is the project's public face and the first thing a potential contributor reads. If it describes capabilities that don't exist or omits capabilities that do, it undermines trust.

#### Scenario: New crate added
- **WHEN** a new crate is added to the workspace (e.g., pane-session for the transport bridge)
- **THEN** the developer SHALL check whether the README's kit/server listing, stack section, or building instructions need updating

#### Scenario: Milestone completed
- **WHEN** a build sequence phase is completed (e.g., Phase 2 transport bridge proven)
- **THEN** the developer SHALL update the README to reflect the new project state


### Requirement: Message type discipline

Pane messages are typed Rust enums serialized with postcard (architecture spec §7). Field access is through Rust struct fields — compile-time verified, no string keys. The review SHALL verify that new message types follow the existing enum conventions and that serialization roundtrips are covered by property tests.

**Hazard**: Adding a message variant without a corresponding proptest roundtrip case means serialization correctness is unverified for that variant.

#### Scenario: New message variant missing proptest coverage
A review catches that a new `TagUpdate` variant was added to the protocol enum but no proptest roundtrip case covers it. The fix: add the variant to the proptest arbitrary implementation.


### Requirement: API stability protocol

All public kit API changes SHALL follow a three-phase deprecation cycle:

1. **Introduce replacement** — new API ships alongside old. Old API annotated `#[deprecated(since = "version", note = "use X instead")]`.
2. **Warn period** — deprecated API produces compile warnings for one release cycle. Migration guide provided.
3. **Remove** — deprecated API removed. Removal is a change proposal, not a silent deletion.

Phase 1 to phase 3 SHALL span at least two release cycles. API additions that expand trait bounds, add required trait methods, or change enum variants in non-`#[non_exhaustive]` types SHALL be treated as breaking changes requiring a change proposal.

**Lesson:** BeOS's BMessage(BMessage*) pointer constructor (Be Newsletter #4-46, Owen Smith) shipped a convenience that enabled implicit conversions and const-correctness violations. By the time it was recognized as a problem, applications depended on it. The deprecation was messy because there was no protocol for it.

#### Scenario: Convenience method enables implicit conversion
- **WHEN** a kit method accepting `impl Into<T>` is found to accept unintended types via blanket impls
- **THEN** the broad signature SHALL be deprecated, a specific signature introduced, and the deprecated version removed after one release cycle


### Requirement: Protocol evolution protocol

Any change to a session type definition in pane-proto SHALL:

1. Be proposed as a change with explicit rationale.
2. Include a compatibility assessment: which crates consume the changed type?
3. Update all consuming crates in the same commit. Partial protocol changes SHALL NOT be committed, even if unchanged consumers happen to compile via wildcard patterns.

**Hazard:** Wildcard match arms (`_ => {}`) on protocol enums silently absorb new variants — a new variant that should trigger behavior gets swallowed. This directly defeats the advantage session types provide over BeOS's convention-based safety.

#### Scenario: New compositor event variant
- **WHEN** a `ScaleChanged` variant is added to the compositor's event enum
- **AND** three kit-level consumers have `_ => { log::warn!(...) }` arms
- **THEN** the review SHALL catch the wildcards and require explicit handling of the new variant


### Requirement: No wildcard arms on protocol enums

Pattern matches on session type enums and protocol message enums defined in pane-proto SHALL NOT use wildcard (`_`) or catch-all arms. Every variant SHALL be matched explicitly.

The entire point of typed protocol enums is exhaustive handling. A wildcard arm converts a compile-time protocol evolution check into a silent runtime swallow. This negates the advantage session types provide over BeOS's convention-based protocol discipline.

**Exception:** Logging/metrics code that genuinely handles "any message" may use wildcards if it does not affect control flow.

#### Scenario: Refactor adds variant, wildcard swallows it
- **WHEN** `FocusStolen` is added to the pane event enum
- **AND** the compositor's per-pane thread has `_ => {}` on events it doesn't handle yet
- **THEN** the review SHALL require an explicit `FocusStolen => {}` arm so the decision to not handle it is visible and reviewable


### Requirement: Performance-sensitive path annotation

Code paths identified as performance-sensitive SHALL be annotated and benchmarked:

1. **Annotated paths:** Message dispatch (looper recv-to-handler), compositor frame assembly, session type send/recv, pane-store query evaluation, FUSE operation handling.
2. **Benchmark suite:** criterion targets for annotated paths. Benchmarks run as part of the review checklist.
3. **Regression threshold:** >10% regression on any annotated benchmark requires explanation. >25% requires a change proposal justification.

Benchmarks measure trends, not correctness. A regression is a signal for investigation, not an automatic block.

**Lesson:** Schillings' benaphore (Be Newsletter #1-26) demonstrated that Be engineers measured at microsecond level. The 35μs→1.5μs difference was 20x and worth an optimization. Hoffman (Be Newsletter #2-36) showed sync vs async calls had measurable performance cliffs.

#### Scenario: Channel implementation change doubles dispatch latency
- **WHEN** the looper's channel implementation is switched from `std::sync::mpsc` to `crossbeam`
- **AND** all tests pass
- **BUT** benchmarks show 2x latency on uncontested recv
- **THEN** the regression SHALL trigger investigation before the change is accepted


### Requirement: Async-default enforcement

The review checklist SHALL include a check for synchronous (blocking) calls in:

- Compositor main loop (calloop callbacks)
- Per-pane server threads during message dispatch
- Client-side looper threads during message handling
- Any code path that executes per-frame or per-input-event

A blocking call in these paths is a review finding of severity **moderate** unless justified. Justified exceptions SHALL be documented with `// SYNC: <reason>` comments.

**What counts as blocking:** `recv()` without timeout on a cross-process channel. `Mutex::lock()` on a lock held by a thread that does I/O. Any filesystem read (generalizes the existing caching invariant). Any network I/O.

**What doesn't count:** `Mutex::lock()` on a component's own state lock (uncontested fast path, per the benaphore lesson). Channel operations within a single process with bounded latency.

**Lesson:** Hoffman (Be Newsletter #2-36): "Synchronous calls are much slower than asynchronous calls... The Interface Kit caches asynchronous calls and sends them in large chunks at a time. A synchronous call requires that this cache be flushed."

#### Scenario: Routing rule evaluation blocks on filesystem read
- **WHEN** the pane-app kit re-reads rule files from disk on each route action
- **THEN** the review SHALL catch this: rules should be cached and updated via pane-notify


### Requirement: Two-failure stop

Two consecutive failures on the same implementation goal — a review verdict of REVISE on the same issue, or a test failure from the same root cause — SHALL trigger a full stop. The agent SHALL:

1. State what it knows, what it doesn't, and what it has tried.
2. Not attempt a third fix.
3. Wait for direction or escalate to a design-level reassessment.

**Lesson:** Schillings didn't keep making semaphores faster — he recognized the approach was wrong and designed the benaphore. The workflow should force the same discipline. Sunk cost pressure causes agents and humans to keep patching forward when the right move is to step back.

#### Scenario: Session endpoint crash handling fails twice
- **WHEN** two attempts to implement crash-safe session teardown both produce panics under race conditions
- **THEN** the agent SHALL stop, document the race condition, and wait for a design-level decision about whether the crash boundary approach is correct
