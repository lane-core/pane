---
type: architecture
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [pane-router, signal-flow, security, policy, firewall, data-management, audit, plumber, MessageFilter]
related: [architecture/kernel, architecture/proto, architecture/session, architecture/app]
agents: [plan9-systems-engineer, be-systems-engineer, session-type-consultant, optics-theorist]
---

# Architecture: pane-router (Signal-Flow Policy)

## Summary

pane-router regulates signal flow between panes. Think
firewall for pane messaging: security policies, data
management rules, audit logging. Configurable at fine grain
(per-signal) or coarse grain (per-pane, per-group).
User-configurable for workflow customization.

## Heritage

- **Plan 9 plumber** — pattern-matched message routing with
  declarative rule files, per-port delivery
- **BeOS BMessageFilter** — per-looper/handler message
  interception (Pass/Skip + retargeting)
- **BeOS BQuery** — live predicate-based filesystem queries

pane-router is genuinely novel: BeOS had no inter-application
signal policy; Plan 9's plumber routed but didn't enforce
security or trigger filesystem effects.

## Placement — Hybrid server + looper

Session-type analysis of four options concluded:

- **(a) Before looper:** conditionally sound, misses outbound
- **(b) Inside server:** conditionally sound, full bidirectional
- **(c) Separate pane:** UNSOUND (cascading ReplyPort failures,
  deadlock via cyclic wait graph)
- **(d) ConnectionSource filter:** UNSOUND for policy

**Decision:** Security + routing policies at server level (b).
Per-pane transform/consume at looper level (a), extending
MessageFilter<M>. Audit at server level (b).

## Router invariants (from session-type analysis)

- **R-I1:** When router blocks a Request, MUST send
  `Failed { token }` to requester. Orphaned DispatchEntry
  otherwise.
- **R-I2:** When router transforms a Request, MUST preserve
  the `token` field. Token is the correlation key.
- **R-I3:** When router defers a Request, deferral queue MUST
  drain on connection teardown.

## MessageFilter<M> gaps

Existing pane-proto MessageFilter (Pass/Transform/Consume) is
insufficient for the router. Three gaps:

1. **No PeerAuth access** — security policies need sender
   identity
2. **No routing/redirect** — FilterAction can't change
   destination
3. **No observation mode** — audit needs to observe without
   deciding

Proposed: separate `AuditHook<M>` trait (write-only, non-
interfering observer). Router policy extends MessageFilter
with PeerAuth context.

## Scope

### What's IN pane-router

- Security ACLs (per-pane, per-protocol, per-PeerAuth)
- Data management rules (MIME-based routing with effects:
  copy, symlink, delete)
- Audit logging (non-interfering AuditSink on server actor)
- Input filter chain (BInputServerFilter heir)
- Declarative rule language (plumb(6)-inspired)

### What's OUT

- Device abstraction (pane-kernel)
- Path resolution (pane-fs)
- Message dispatch (pane-app)
- Wire protocol (pane-session)

## Optics analysis

- Routing decisions (pass/consume/redirect) are NOT optics —
  they're control flow
- Rule predicates are Boolean algebra over Getters, not
  Traversals
- Message transformation within rules IS a Setter/over
- Router ACL on state projection: Lens ∘ Prism =
  AffineTraversal (clean composition)
- Router effects (copy, symlink, delete) should NOT flow
  through MonadicLens's Vec<Effect> — separate RouterEffect
  type

## Audit channel

Non-interfering observer on the server actor. Write-only
`AuditSink` trait, bounded channel with try_send (drop on
full — audit loss tolerable, routing stall is not). Not
session-typed (fire-and-forget, no protocol to enforce).
Clone-before-dispatch for observation (I5 Message Clone
bound enables this; obligation handles are !Clone and
correctly excluded from audit content).

## Provenance

Design established 2026-04-12 via four-agent roundtable.
Router is a new design with no single-kit Be heritage —
it synthesizes BMessageFilter (signal interception), BQuery
(predicate matching), and the plumber (declarative routing)
into a unified policy layer.

## See also

- `architecture/kernel` — pane-kernel system interface
- `architecture/proto` — MessageFilter<M>, FilterAction
- `architecture/session` — ProtocolServer actor (enforcement point)
- `decision/host_as_contingent_server`
