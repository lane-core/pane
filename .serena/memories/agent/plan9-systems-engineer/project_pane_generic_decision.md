---
name: Pane generic parameter decision — PaneBuilder<H> approved
description: PaneBuilder<H> over Pane<H>: namespace analogy, type erasure, distributed consequences, naming rationale, service handle binding semantics (2026-04-03). Name evolved PaneSetup→PaneInit→PaneBuilder.
type: project
---

Consultation on Pane generic parameter, two rounds (2026-04-03).

**Note:** Final name is `PaneBuilder<H>` (evolved from PaneSetup→PaneInit→PaneBuilder after all three agents agreed on "Builder" as the most accurate Rust convention).

## Decision: PaneBuilder<H> pattern (originally named PaneInit)

Three-agent deliberation concluded with Option C (generic setup phase, non-generic Pane). Plan 9 review confirmed soundness.

## Round 1: PaneSetup<H> recommended (pre-naming)

Lane asked for Plan 9 distributed-computing analysis. Six-point analysis:
1. Location transparency: H never crosses the wire. Setup erases H at the natural boundary.
2. Namespace projection: pane-fs needs uniform pane collections. Non-generic Pane gives Vec<Pane>.
3. Service map: Equivalent either way. H doesn't affect resolution.
4. Cross-pane communication: Senders don't know receiver's H.
5. 9P analogy: Plan 9 separates namespace construction (bind/mount) from running process. Setup<H> = namespace construction; run_with = exec; Pane = running process.
6. Server management: Server doesn't run handlers, can't hold Pane<H>.

## Round 2: Full review, naming resolved to PaneInit<H>

Reviewed complete proposal including API surface, invariants, and five specific questions.

### Key findings
1. **Location transparency sound.** One edge: PaneInit may accumulate multiple Connection references during setup when services resolve to different servers via service map.
2. **Heterogeneous collection issue fully resolved.** Vec<Pane> works, no trait objects.
3. **Add run_with/run_with_display shortcuts directly on Pane.** No distributed concerns — skipping setup is just exec without prior bind/mount. Common case shouldn't force ceremony.
4. **Named PaneInit<H>, not PaneConfig or PaneSetup.** Init = one-time, irreversible, does real I/O (DeclareInterest), consumed by run_with. Config implies declarative/reusable (wrong). Setup implies setup/teardown symmetry (wrong — consumed, not torn down). Init matches Plan 9 init semantics and Rust #[must_use] obligation.
5. **ServiceHandle<P> bound to Connection+version at open time.** Service map changes affect new opens, not existing handles. This is Plan 9 fid semantics (fid bound at open, mount table changes affect new walks only). Transparent failover (service follows handle) would be a separate opt-in mechanism.

**Why:** Lane needed to resolve the type-level gap where open_service needs H: Handles<P> but Pane was non-generic. Three agents recommended Option C; Plan 9 review for completeness and distributed soundness.

**How to apply:** Update architecture.md: PaneInit<H> struct, #[must_use], Drop compensates DeclareInterest. Add run_with/run_with_display shortcuts on Pane. State ServiceHandle binding semantics in Service Registration section.
