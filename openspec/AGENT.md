# Agent Implementation Guide

You are implementing pane — a Wayland compositor and desktop environment
for Linux, guided by sequent calculus / CBPV theory (Value/Compute polarity).
The project has a structured specification system (OpenSpec) that defines
contracts, tracks changes, and provides enriched context for implementation.

**Read CLAUDE.md first** — it is the authoritative project instruction
file and takes precedence over everything here. This file supplements
CLAUDE.md with openspec-specific workflow guidance.


## Orientation

### What exists

| Layer | What | Where |
|-------|------|-------|
| Architecture | Vision, design pillars, server/kit decomposition, build sequence | openspec/specs/architecture/spec.md |
| Protocol contracts | PaneMessage, PaneRequest/PaneEvent, state machine, polarity markers | openspec/specs/pane-protocol/spec.md |
| Cell grid contracts | Cell, Color, CellAttrs, CellRegion, input events | openspec/specs/cell-grid-types/spec.md |
| Compositional interfaces | Result-like combinators, Proto combinator, reactive signals, boundaries | openspec/specs/compositional-interfaces/spec.md |
| Workflow | Development protocols: review, testing, escalation, caching invariant | openspec/specs/workflow/spec.md |
| **Specs** | All normative contracts (SHALL/MUST) with verification criteria | openspec/specs/ |
| **Changes** | Scoped implementation plans with tasks | openspec/changes/ |
| **Schema** | Custom `pane` workflow: proposal → contracts → architecture → tasks | openspec/schemas/pane/ |


## How to use the tooling

### Starting a task

```sh
# See what's available
openspec list --specs        # all specs and requirement counts
openspec list                # active changes and task progress

# Get context for implementation
openspec show <change>                          # read proposal + architecture
openspec instructions tasks --change <change>   # enriched task context
openspec show --type spec <spec-name>           # read a spec's contracts

# Check current state
openspec status --change <change>               # artifact completion
openspec validate --all                         # structural validation
```

### Creating a change

Always use the pane schema:
```sh
openspec new change --schema pane "<name>"
```

This produces: proposal → contracts + architecture (parallel) → tasks.

### Implementation loop

1. Pick the next unchecked task from `tasks.md`
2. **Read the relevant spec(s) for contracts that apply** — contracts are law
3. Read the architecture document for design decisions and rationale
4. Implement. Every completed task group must pass `cargo build && cargo test`
5. Mark the task `- [x]` in tasks.md
6. Follow the pre-commit review protocol (workflow spec §Pre-commit review)

### Marking progress

Edit the tasks.md file directly — change `- [ ]` to `- [x]` as you
complete each task. The openspec apply phase tracks this.


## Contracts are law

The specs define behavioral contracts using SHALL/MUST language. These
are not suggestions:

- **SHALL** = mandatory behavior. The implementation MUST produce this.
- **MUST** = implementation constraint. The code MUST satisfy this.
- **Hazard** = non-obvious failure mode. Read these carefully.
- **Polarity** = Value (constructed data), Compute (observed behavior), or Boundary.
- **Crate** = where to find the authoritative implementation.

Each contract has a `#### Scenario:` verification criterion. Use these
as your acceptance tests.


## Workflow protocols (from workflow spec)

The workflow spec (`openspec/specs/workflow/spec.md`) defines requirements
covering how you work. The critical ones:

- **Pre-commit review**: every commit gets a correctness review. Non-trivial
  changes spawn a `feature-dev:code-reviewer` agent.
- **Zero-failure test policy**: any test failure is YOUR regression until
  proven otherwise. Test the pre-change state before claiming pre-existing.
- **Discovery-driven restart**: if you discover a new invariant mid-work,
  re-evaluate the design rather than patching forward.
- **Contract-first implementation**: read contracts BEFORE writing code.
  Implement what the contract says, not what seems reasonable without checking.
- **Filesystem caching invariant**: servers cache config in memory, update
  only on pane-notify events. Never do filesystem I/O in the render loop.
- **Polarity consistency**: new types get polarity annotations. Polarity
  changes trigger consumer review.
- **Inter-server protocol discipline**: use typed views, not raw attr access.
- **TODO.md capture**: noticed issues go in TODO.md, not fixed inline.
- **Approach-level escalation**: if the review reveals the approach is wrong,
  stop and redesign. Don't patch a wrong approach.


## Build validation

```sh
# Standard validation
LIBRARY_PATH="$(xcrun --show-sdk-path)/usr/lib" cargo test    # macOS dev (temporary)
cargo build && cargo test                                      # Linux (target platform)

# After completing a task group
cargo build && cargo test    # MUST pass before marking group complete
```

Note: The macOS LIBRARY_PATH workaround is temporary — pane targets Linux.
When developing on macOS, ensure the workaround is applied. On Linux,
standard cargo commands work directly.


## Key specs by build phase

### Phase 1 (pane-proto) — COMPLETE ✓
- `pane-protocol` spec: message types, state machine, PaneMessage wrapper, polarity markers
- `cell-grid-types` spec: Cell, Color, CellAttrs, CellRegion, input events

### Phase 2 (pane-notify)
- `pane-notify` spec (in architecture): fanotify/inotify abstraction, calloop integration

### Phase 3 (pane-comp skeleton)
- `pane-compositor` spec (in pane-comp-skeleton change): winit backend, calloop event loop, chrome rendering
- `cell-grid-renderer` spec (in pane-comp-skeleton change): glyph atlas, cell rendering, font loading

### Phase 4 (pane-shell)
- Architecture spec §pane-shell constraints: xterm-256color, screen buffer, dirty regions, alternate screen

### Phase 5+ (servers)
- Architecture spec §Servers: pane-route, pane-roster, pane-store, pane-fs
- Architecture spec §Filesystem-Based Configuration, §Filesystem-Based Plugin Discovery


## Things that will trip you up

1. **macOS dev environment needs LIBRARY_PATH** for libiconv. Set
   `LIBRARY_PATH="$(xcrun --show-sdk-path)/usr/lib"` before cargo test.
2. **CellRegion requires validated construction** via `CellRegion::new()`.
   Direct struct construction will fail — `cells` field is in the struct
   but `width * height` invariant is checked by the constructor.
3. **FKey requires TryFrom<u8>** — values outside 1-24 are rejected.
   Proptest strategies must generate valid values.
4. **ProtocolState is NOT serializable** — no Serialize/Deserialize.
   It's local per-connection tracking.
5. **PaneEvent::Created now includes `kind`** — consumers must handle
   the kind field (added during multi-pane redesign).
6. **PlumbMessage is gone** — renamed to RouteMessage. TagPlumb → TagRoute.
   Plumb → Route. pane-plumb → pane-route.
7. **frame() now returns Result** — it was infallible before, now errors
   on payloads > u32::MAX.
8. **pane-input is not a server** — input handling is a module within
   pane-comp. Don't create a pane-input crate.
