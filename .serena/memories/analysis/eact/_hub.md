---
type: hub
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [eact, fowler, hu, session_types, preservation, progress, global_progress, conditional_soundness, multiparty, actor, watchdog, I2, I3]
related: [reference/papers/eact, reference/papers/eact_sections, reference/papers/dlfactris, analysis/session_types/_hub, architecture/looper, decision/server_actor_model, decision/wire_framing]
agents: [session-type-consultant, formal-verifier, pane-architect]
---

# EAct analysis cluster

## Motivation

Pane's actor framework claims EAct (Fowler & Hu, *Safe Actor
Programming with Multiparty Session Types*) and DLfActRiS
(Hinrichsen/Krebbers/Birkedal) as its formal ground. This
cluster audits that claim — property by property, divergence
by divergence, invariant by invariant — so the claim is
defensible in current code and unambiguous for future
changes.

All three EAct theorems (Preservation, Progress, Global
Progress) hold for pane **conditionally**: watchdog
enforcement for I2/I3 (EAct cannot rule out blocking or
non-terminating handlers at the type level), bilateral-only
sessions by design (no multiparty projection on the wire),
and the single-mailbox actor invariant implemented by
`ProtocolServer` (see `decision/server_actor_model`). The
audit is *load-bearing* for looper changes, dispatch edits,
and destruction-sequence refactors.

## Spokes

- [`analysis/eact/audit_2026_04_05`](audit_2026_04_05.md) —
  property mapping (EAct Theorems 4–8) against pane.
  Conditionally sound with three enforcement sites
  identified. Some 2026-04-05 invariant status notes are
  partially stale (superseded by later spokes).
- [`analysis/eact/divergences_2026_04_06`](divergences_2026_04_06.md)
  — four formal divergences (E-CancelMsg, E-Spawn, ibecome,
  E-Monitor) vetted 4–0 by the session-types roundtable;
  E-Monitor is now implemented (watch / unwatch /
  PaneExited, commit `e5cd130`).
- [`analysis/eact/gaps`](gaps.md) — seven structural gaps
  (sub-protocols, multi-source dispatch, conversation
  failure, cascading, ABA, type confusion, death
  notification) and the resolutions that landed in the
  architecture spec.
- [`analysis/eact/invariants`](invariants.md) — type-level
  and convention-level enumeration of I1–I13 + S1–S6 with
  test-status mapping. The implementer's checklist.
- [`analysis/eact/design_principles_not_adopted`](design_principles_not_adopted.md)
  — explicit non-adoptions (effect system, Scribble, dynamic
  linearity, suspend / become, access points as separate
  layer, multiparty-on-wire) with rationale. Read this
  before proposing a "let's add X from EAct" change.

The 2026-04-06 formal-verifier session report (covering
invariants, deadlock, polarity, and test gaps) lives in
`analysis/verification/session_audit_2026_04_06` — it is
the invariant-status source for I1–I13 at that date. Cross-
reference from here; it is not an eact spoke because its
scope is broader than EAct alone.

## Open questions

- **I2/I3 timeout watchdog** — architecture/looper owns the
  watchdog source; thresholds and handler-specific overrides
  are still convention, not type-checked. Halting problem
  means EAct can't rule these out at the language level;
  pane accepts the detection-enforced form.
- **I12 soft-drop vs spec** — permissive codec + looper-level
  soft-drop is the current implementation, but the
  architecture spec still describes a connection-level
  error. Reconcile during the next spec pass.
- **Multiparty reasoning for 3+ service protocols** — pane
  deliberately runs binary session types on the wire, but
  design-time multiparty checking (project to binary,
  encode per tag) is a candidate tool for future service
  protocol design.

## Cross-cluster references

- `reference/papers/eact` — Fowler & Hu primary source;
  all property and rule numbering traces here.
- `reference/papers/eact_sections` — page-anchored digest for
  fast lookup.
- `reference/papers/dlfactris` — deadlock-freedom theorem
  for actor networks; star topology proof pane relies on.
- `analysis/session_types/_hub` — general session-type
  design discipline, adjacent cluster.
- `analysis/verification/_hub` — invariant coverage audits,
  including the 2026-04-06 session report.
- `architecture/looper` — implementation site for I1, I2,
  I3, I8, I9, S3, S4, S6 enforcement.
- `decision/wire_framing` — S8 protocol tag + S9 session_id
  widening, informed by EAct polarity discipline.
- `decision/server_actor_model` — single-mailbox actor
  grounding referenced throughout the cluster.
