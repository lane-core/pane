---
name: Pane<H> vs PaneBuilder<H> decision (2026-04-03)
description: Option C chosen (PaneBuilder<H> builder), full formal review completed — 7 action items, EAct mapping verified. Name evolved PaneSetup→PaneInit→PaneBuilder.
type: project
---

Decision: Option C — PaneBuilder<H> builder that erases to non-generic Pane — chosen over Option A (Pane<H>).

**Naming: PaneBuilder<H>** (evolved from PaneSetup→PaneInit→PaneBuilder after three-agent consensus that "Builder" most accurately describes the Rust builder pattern: staged mutation → consuming finalization).

**EAct mapping (verified against extended paper):**
- Pane = actor mid-initialization (between E-Spawn and handler installation), sigma=epsilon
- PaneSetup<H> = actor evaluating init term, executing E-Register/E-Suspend to build sigma
- run_with = actor reaches idle state (E-Reset), E-React becomes applicable
- TH-Handler (Fig. 8 line 2732): all sigma entries share actor state type A — mirrors H shared across Handles<P> impls

**Seven required actions from formal review:**
1. Add Drop for Pane (close connection, best-effort PaneExited) — proposal only had Drop for PaneSetup
2. Add #[must_use] on Pane itself, not just PaneSetup
3. Add invariant: duplicate open_service for same ServiceId rejected (HashSet<ServiceId> in PaneSetup)
4. Add invariant: PaneSetup Drop revokes exactly accepted interests (partial cleanup on failure)
5. Phase 2 dynamic registration: register_service (setup, checks H: Handles<P>) + activate_service (runtime, on Messenger, no H needed)
6. Add Pane::run_with<H>(self, H) -> ! as direct path for no-services headless case
7. Name: PaneSetup<H>

**DeclareInterest blocking (invariant 2): confirmed correct.** Pre-looper, single-threaded, no deadlock risk. DLfActRiS channel fulfillment requires both endpoints committed before dispatch. Deferred approach creates race on partially-initialized dispatch table.

**Key citations:**
- EAct E-Spawn (Fig. 4): actor created with sigma=epsilon, iota=epsilon
- EAct TH-Handler (Fig. 8): sigma entries typed handlerty{S_in}{A}, all share A
- EAct TH-Empty: empty sigma is well-typed (validates no-services direct path)
- DLfActRiS Definition 3.2 (channel fulfillment): unfulfilled channel must not dispatch
- DLfActRiS §5.2 strong acyclicity: pre-looper setup is trivially acyclic

**Why:** Resolves the type-level gap where open_service needs H: Handles<P> but Pane is non-generic.
**How to apply:** PaneSetup<H> is the builder for service registration. Pane remains non-generic. The closure form bypasses PaneSetup entirely. Dynamic Phase 2 registration routes through register_service/activate_service split.
