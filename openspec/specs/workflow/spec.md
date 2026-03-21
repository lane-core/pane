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
While implementing the cell grid renderer, the agent notices that CellRegion doesn't validate width > 0. It adds a TODO.md entry and continues with the renderer.


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


### Requirement: Message type discipline

Pane messages are typed Rust enums serialized with postcard (architecture spec §7). Field access is through Rust struct fields — compile-time verified, no string keys. The review SHALL verify that new message types follow the existing enum conventions and that serialization roundtrips are covered by property tests.

**Hazard**: Adding a message variant without a corresponding proptest roundtrip case means serialization correctness is unverified for that variant.

#### Scenario: New message variant missing proptest coverage
A review catches that a new `TagUpdate` variant was added to the protocol enum but no proptest roundtrip case covers it. The fix: add the variant to the proptest arbitrary implementation.
