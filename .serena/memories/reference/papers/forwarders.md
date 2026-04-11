---
type: reference
status: current
created: 2026-04-10
last_updated: 2026-04-10
importance: high
keywords: [carbone, forwarders, multiparty, compatibility, MCLL, cut_elimination, linear_logic]
related: [reference/papers/_hub, reference/papers/eact, policy/agent_workflow]
agents: [session-type-consultant, plan9-systems-engineer]
---

# Forwarders: A logical interpretation of asynchronous multiparty compatibility

**Authors:** Carbone, Marin, Schürmann
**Path:** `~/gist/logical-interpretation-of-async-multiparty-compatbility/`

## Summary

Frames multiparty compatibility through linear logic. The
**forwarder** construction (a process `fwd a b` that links two
channels) captures all multiparty-compatible compositions and
preserves cut-elimination. This is the foundation for proving
that protocol composition is sound.

The key theorem (§5.1) shows cut on forwarder chains reduces
cleanly — meaning a chain of intermediaries between a sender
and ultimate recipient does not deadlock if each link is
forwarder-correct.

## Concepts informed

- Star topology (one shell-as-hub forwarding to many) is sound
- Backwards-compat via thin forwarder stubs during migration
  (the forwarder construction in serena clothes)
- Cut-admissibility for MCLL (multiplicative classical linear
  logic)
- The proof structure for "forwarder chain composes" applies to
  pane's server-mediated routing model

## Used by pane

- `reference/plan9/divergences` — pane-server as forwarder
- `reference/papers/eact` cross-reference — DLfActRiS uses the
  forwarder construction
- `policy/agent_workflow` — session-type consultant cites this
  paper when validating star topologies
