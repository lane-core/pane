---
type: project
status: open
created: 2026-04-11
last_updated: 2026-04-11
importance: medium
keywords: [DLfActRiS, global_progress, direct_pane_to_pane, session_types, star_topology, theorem_scope]
related: [decision/server_actor_model, decision/messenger_addressing, agent/plan9-systems-engineer/project_connectionsource_review, reference/papers/dlfactris]
agents: [plan9-systems-engineer, session-type-consultant, formal-verifier]
---

# Flag: DLfActRiS Theorem 1.2 re-verification for Phase 2

Raised 2026-04-11 during ConnectionSource design review.

## The concern

`decision/server_actor_model` cites Jacobs/Hinrichsen/Krebbers
(POPL 2024) Theorem 1.2 for global progress. That theorem is
proved for a **star topology** — actor at center, connections
as leaves. The citation is correct in the current Phase 1
architecture where the ProtocolServer is the sole center.

`decision/messenger_addressing §2` commits Phase 2 to **direct
pane-to-pane communication**, bypassing the server. This takes
the connection graph off the star. DLfActRiS Theorem 1.2 as
cited no longer transitively covers the full system.

## Why this may not matter

The request-wait graph (not the connection graph) is what needs
to be acyclic for progress. pane's dispatch model enforces
acyclicity of the request-wait graph by construction:

1. **I2** — handlers cannot block, so no handler holds the
   dispatch thread waiting for a reply
2. **I8** — send_and_wait panics from the looper thread, so
   synchronous waits only happen on non-looper threads
3. **Protocol-scoped send_request** — session types constrain
   legal messages per state; deadlock-free session types give
   deadlock-free dispatch

If the dispatch model alone suffices, no topology-specific
theorem is needed, and direct pane-to-pane is fine.

## What session-type-consultant should decide

Either:

**(a)** A different theorem covers direct-to-direct
(deadlock-freedom for arbitrary session-typed actor graphs
under restricted protocols), and we cite that instead. Look at
EAct (Fowler/Hu), multiparty session types, or forwarders
(`reference/papers/forwarders`).

**(b)** I2 + I8 + session-type dispatch is itself the argument,
and DLfActRiS is retired or restricted to "this is how the
ProtocolServer's local invariants work" rather than a
whole-system progress claim.

**(c)** The direct pane-to-pane feature needs additional
runtime or type-level constraints to remain safe.

## Where to revisit

- When Phase 2 distribution work opens and direct pane-to-pane
  moves from design to implementation
- When formal-verifier next audits invariants against the
  session-types crate
- If a user of pane hits a deadlock that I2/I8 doesn't catch

## Not a blocker for ConnectionSource

ConnectionSource itself should not enforce any topology
constraint. This flag concerns the *theorem citation* in
`decision/server_actor_model`, not the ConnectionSource
implementation.
