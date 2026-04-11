---
type: reference
status: current
sources: [.claude/agent-memory/session-type-consultant/reference_eact_paper]
created: 2026-04-10
last_updated: 2026-04-10
importance: normal
keywords: [eact, fowler, hu, maty, maty_zap, theorem_locations, preservation, progress, global_progress, e_send, e_react, e_suspend, KP1, KP2, KP3, KP4, KP5]
related: [reference/papers/_hub, reference/papers/eact, reference/papers/forwarders, reference/papers/dlfactris]
agents: [session-type-consultant, formal-verifier, pane-architect]
---

# EAct paper — section / theorem locator

Companion to `reference/papers/eact`. The high-level summary
lives there; this memory is the deep section map for citing
specific theorems and reduction rules.

**Paper:** "Speak Now: Safe Actor Programming with Multiparty
Session Types" (Extended Version)
**Authors:** Simon Fowler (Glasgow), Raymond Hu (QMUL)
**Source:** `~/gist/safe-actor-programming-with-multiparty-session-types/eventactors-extended.tex`
**Language name:** Maty (base), Maty_zap (with failure)

## Key theorems and definitions

- **Definition 3** (Compliance, §3.3) — safe + deadlock-free runtime type environments
- **Theorem 4** (Preservation, §3.3) — typability preserved by structural congruence and reduction
- **Remark 1** (Session Fidelity, §3.3) — fidelity as corollary of preservation
- **Theorem 5** (Progress, §3.3) — well-typed closed config either reduces or is done
- **Definition 5** (Canonical form, §3.3) — normal form for non-reducing configs
- **Definition 7** (Thread-Terminating, §3.3) — actor thread eventually reaches idle or suspend
- **Lemma 1** (Independence of Thread Reductions, §3.3) — thread reduction in one actor doesn't inhibit another
- **Corollary 2** (§3.3) — thread-terminating configs reach idle
- **Lemma 2** (§3.3) — every ongoing session in idle config can reduce
- **Corollary 1** (Global Progress, §3.3) — thread-terminating ⇒ every ongoing session eventually communicates
- **Theorem 6** (Preservation under failure, §4) — preservation with cancellation-aware environments
- **Theorem 7** (Progress under failure, §4) — progress with zapper threads
- **Theorem 8** (Global Progress under failure, §4) — sessions either communicate or are fully cancelled

## Key reduction rules

- **E-Send** (§3.2) — async message append to session queue
- **E-React** (§3.2) — idle actor + stored handler + queued message ⇒ handler invoked
- **E-Suspend** (§3.2) — actor installs handler and returns to idle
- **E-Init** (§3.2) — access point establishes session when all roles registered
- **E-RaiseS** (§4) — exception in session context ⇒ zapper threads for actor + role
- **E-CancelMsg** (§4) — messages to cancelled role are discarded
- **E-CancelH** (§4) — handler waiting on cancelled sender ⇒ failure continuation
- **E-InvokeM** (§4) — monitor callback fires when monitored actor crashes

## Key principles (§1.3)

- **KP1** Reactivity — computation triggered by incoming messages
- **KP2** No Explicit Channels
- **KP3** Multiple Sessions per actor
- **KP4** Interaction Between Sessions
- **KP5** Failure Handling and Recovery

## Affine sessions citation

§4 cites Mostrous / Vasconcelos 2018, Harvey et al. 2021,
Lagaillardie et al. 2022, Fowler et al. 2019.

## Use

When citing EAct in a pane analysis or memo, use this locator
to find the exact theorem / rule. The high-level summary in
`reference/papers/eact` is for orientation; this is the deep
reference for formal arguments.
