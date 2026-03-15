# Workflow: Development Protocols

Process invariants for how changes to pane are proposed, implemented, verified, reviewed, and committed. These are as load-bearing as behavioral contracts — violating them has caused real regressions and lost work in analogous projects.

## Purpose

Development, testing, review, and commit protocols that govern how changes to pane are validated and integrated. These exist because pane is a compositor (crashes take down the display), a protocol system (bugs propagate across process boundaries), and a distro foundation (regressions compound).

## Requirements

### Requirement: Change-driven development

All non-trivial work SHALL go through the openspec change workflow: propose → contracts → architecture → tasks → apply → archive. Trivial fixes (typos, single-line bug fixes) may bypass the workflow but MUST still follow the review protocol.

The custom `pane` schema SHALL be used for all changes (`openspec new change --schema pane "<name>"`). Contracts and architecture are parallel — both depend on the proposal but not on each other.

**Polarity**: Boundary
**Crate**: N/A (process)

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

#### Scenario: New protocol type triggers agent review
Adding a new variant to PaneRequest spawns a `feature-dev:code-reviewer` agent before any `git add`.


### Requirement: Review checklist

The review SHALL verify five items in order:

1. **Task completion** — diff accomplishes what was requested. Flag claimed-but-missing or present-but-unrequested changes.
2. **Contract compliance** — cross-reference against `openspec/specs/`, architecture spec, CLAUDE.md. Run `openspec validate --all`.
3. **Reference accuracy** — every type name, module path, crate reference, or cross-reference in the diff MUST be verified against current codebase state.
4. **Approach validity** — for non-trivial changes: is this the right approach? Does it respect polarity contracts? Could it fail for reasons not visible in test results?
5. **Build and test** — `cargo build && cargo test` MUST pass. New warnings MUST be acknowledged or fixed. Test count MUST not regress.

#### Scenario: Stale module path caught
A doc comment citing `pane_proto::message::PlumbMessage` (old name) is flagged because the type was renamed to `RouteMessage`.


### Requirement: Review verdict

The review SHALL produce one of three verdicts:

- **PASS** — all checks satisfied, proceed to commit.
- **PASS with notes** — minor issues that don't block. Notes go in commit message or TODO.md.
- **REVISE** — issues listed with severity (critical/moderate/minor) and concrete fixes. MUST NOT commit until critical and moderate issues are resolved.

#### Scenario: Critical protocol bug blocks commit
A review finding that the state machine accepts WriteCells on a Surface pane produces REVISE with severity critical.


### Requirement: Zero-failure test policy

Any test failure SHALL be treated as a regression until proven otherwise. The agent SHALL assume it caused any failure observed during its work. The burden of proof is on the agent to demonstrate a failure is pre-existing (by testing the pre-change state).

#### Scenario: Proptest failure during state machine work
A proptest failure in `roundtrip_pane_request` during protocol changes is assumed to be caused by those changes. The agent tests the pre-change state before claiming pre-existing.


### Requirement: Discovery-driven restart

When a previously-unknown invariant or contract is discovered mid-implementation, it SHALL be treated as new spec, not as a bug in the current code. The agent SHALL re-evaluate design choices against the expanded specification before patching forward.

If the new invariant would have changed how the code was structured, restart from the expanded spec rather than retrofitting.

**Hazard**: Patching forward after discovering a missed invariant produces brittle code that technically works but structurally ignores the constraint.

#### Scenario: New polarity constraint discovered
Discovering that Async.and_then(Value) requires explicit synchronization triggers a redesign of the Proto combinator, not a local workaround.


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

**Hazard**: Writing code that "seems right" without checking contracts leads to implementations that pass tests but violate invariants not covered by tests.

#### Scenario: Protocol state machine implementation
Before modifying ProtocolState, the agent reads the pane-protocol contract to verify which transitions are valid, what errors are expected, and what polarity constraints apply.


### Requirement: Filesystem caching invariant enforcement

Any code that reads configuration or filesystem state in a server's event loop SHALL be reviewed for caching invariant compliance. Servers cache filesystem state in memory at startup and update only in response to pane-notify events. The render loop and event dispatch SHALL NOT perform filesystem I/O.

**Hazard**: A single `fs::read()` in the render loop at 60fps produces 60 syscalls/second per config key.

#### Scenario: Config read in render path caught
A review catches `std::fs::read_to_string("/etc/pane/comp/font")` inside the frame rendering function. The fix: read on startup and on pane-notify event, cache in memory.


### Requirement: Polarity consistency

Protocol types SHALL maintain consistent Value/Compute polarity annotations. When a new type is added, it SHALL be annotated with the appropriate polarity trait. When an existing type's polarity semantics change, all consumers SHALL be reviewed for compatibility.

**Polarity**: Boundary
**Crate**: `pane-proto::polarity`

#### Scenario: New request variant gets Value annotation
Adding a new variant to PaneRequest includes `impl Value for PaneRequest` (already blanket) but the review verifies the variant's semantics are Value-compatible (constructed data, not behavior-defined).


### Requirement: Inter-server protocol discipline

Inter-server messages SHALL use `PaneMessage<ServerVerb>` with typed views for field access. Raw attr key access (`msg.attr("key")`) SHALL NOT be used in production code paths — only in typed view `parse()` implementations. This prevents stringly-typed field access from leaking beyond the parsing boundary.

**Hazard**: Direct attr access outside typed views reintroduces the BMessage typo problem that typed views exist to prevent.

#### Scenario: Direct attr access in server logic flagged
A review catches `msg.attr("action").unwrap()` in pane-route's dispatch logic instead of `RouteCommand::parse(&msg)?.action()`. The fix: use the typed view.
