---
name: Pane generic parameter analysis
description: Option C (PaneBuilder<H> builder erasing to non-generic Pane) recommended over Pane<H> — 6-point distributed systems analysis (2026-04-03). Name evolved PaneSetup→PaneInit→PaneBuilder.
type: project
---

Analysis of whether Pane should be generic over its handler type H (2026-04-03).

**Note:** Final name is `PaneBuilder<H>` (evolved from PaneSetup→PaneInit→PaneBuilder).

## Decision: Option C (PaneBuilder<H> → non-generic Pane)

Recommended over Option A (Pane<H>) across all six evaluation dimensions.

## Key findings

1. **Location transparency.** Handler type has no wire representation. Server allocates pane slots without knowing H. Pane<H> bakes a local dispatch detail into a type that conceptually crosses machine boundaries. PaneSetup<H> keeps H in the setup window only.

2. **Namespace projection.** /pane/ needs uniform entries (like Plan 9 /proc/). Pane<H> forces Vec<Box<dyn ErasedPane>> at every collection boundary. PaneSetup<H> → Pane gives Vec<Pane> directly.

3. **Service map.** Resolution uses ServiceId (UUID), not handler type. Both options can enforce H: Handles<P> at open_service. But Pane<H> creates temptation to condition resolution on trait bounds, inverting the namespace model.

4. **Cross-pane communication.** Messenger is already non-generic. Protocol boundaries are serialized bytes + service discriminant. Pane<H> is dead weight at this boundary.

5. **9P analogy.** Capabilities belong in the namespace (DeclareInterest), not the type (trait bounds). PaneSetup<H> checks bounds during assembly, then capabilities live in protocol state. Pane<H> permanently encodes capabilities in the Rust type.

6. **Server runtime.** Vec<Pane> vs Vec<Box<dyn ErasedPane>>. H lives inside the looper thread after run_with(), invisible to external code.

## Open question

Whether PaneSetup needs the turbofish at create_pane::<Editor>() or can defer H inference to run_with(). Rust trait solver ergonomics question, not a distributed systems question.

**Why:** Lane asked for Plan 9 perspective on type-level gap between non-generic Pane and H-bounded open_service.

**How to apply:** Reference when implementing Phase 1 Kit API. PaneSetup<H> is the setup-phase struct. Pane is the runtime handle. H is consumed by run_with() and lives only inside the looper thread.
