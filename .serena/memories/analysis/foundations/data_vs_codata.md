---
type: analysis
status: current
audited_by: [optics-theorist@2026-04-11]
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [data, codata, constructors, destructors, pattern_match, copattern_match, observers, duality, service_frame, handles, attr_reader]
related: [analysis/foundations/_hub, analysis/foundations/error_handling_duality, analysis/optics/writer_monad, analysis/optics/implementation_guidance, reference/papers/grokking_sequent_calculus]
sources: [../psh/.serena/memories/analysis/data_vs_codata]
verified_against: [../psh/docs/vdc-framework.md@2026-04-11]
agents: [optics-theorist, formal-verifier, pane-architect]
---

# Data vs codata (constructors vs destructors)

## Problem

Pane has both kinds of types but has not named the duality:
some types are defined by their constructors (build one, pick
a variant, hand it off) and others by their destructors
(receive observations, respond to each one). The distinction
governs which eliminator is legal, which side picks, and which
category the type lives in. Without naming it, the duality
between `ServiceFrame` and `Handles<P>` looks incidental.

## Resolution

The sequent calculus makes **data types** and **codata types**
perfectly dual:

- **Data types are defined by their constructors.** The
  consumer must pattern-match. The producer chooses which
  constructor to use; the consumer must handle every case. In
  pane, `ServiceFrame` is data: variants `Request`, `Reply`,
  `Failed`, `Notification`. The looper's frame-decoder is the
  pattern-match site; every variant must be handled.
  `ControlMessage` is data for the same reason. `ExitReason`
  is data.
- **Codata types are defined by their destructors.** The
  producer must handle all observations. The consumer chooses
  which destructor to invoke; the producer must respond. In
  pane, `Handles<P>` is codata: the destructor is `receive`,
  and the `Handler` lifecycle surface extends this with
  `ready`, `disconnected`, `pulse`, `close_requested`. The
  *consumer* (the looper / the transport) picks which
  destructor fires; the *producer* (the handler
  implementation) must respond. `AttrReader<S>` is codata: the
  destructor is the view function, the producer is the state
  snapshot that responds when the view is applied.

Quoting psh's framing directly (psh's `docs/vdc-framework.md`
§3.5, line 310 in psh's tree): "The sequent calculus makes
this duality first-class: constructors and destructors,
pattern matches and copattern matches, are symmetric. Rc does
not formalize this symmetry, but it is already present in the
design." Pane's situation is the same: the symmetry is present
in the design (via `#[non_exhaustive]` enums for data and
trait-method dispatch for codata), but the vocabulary has not
been adopted in the spec.

The duality surfaces in two places already covered by existing
analysis:

- `analysis/optics/writer_monad` — `MonadicLens<S, A>` with
  pure view (the codata destructor) and effectful set is
  exactly the optic class that fits a codata-style variable.
- `analysis/foundations/error_handling_duality` — ⊕ (status,
  caller-inspects) is a data type; ⅋ (trap / failure
  callback, callee-invokes) is a codata type. Same duality,
  different axis.

## Status

Adopted from psh 2026-04-11; treat as design vocabulary. When
asked whether a new type should be data or codata, the test is:
who picks the elimination form? Producer picks (constructors)
→ data. Consumer picks (destructors) → codata. The caveat psh
carries — "Rc does not formalize this symmetry, but it is
already present in the design" — applies to pane: the symmetry
is present, not formalized.

## Source

- `../psh/docs/vdc-framework.md` §3.5 "Data and codata" lines
  277–310 — psh's framing with rc examples (argument list as
  data, fd as codata). Not vendored into pane; read from the
  psh checkout.
- `reference/papers/grokking_sequent_calculus` — Binder,
  Tzschentke, Müller, Ostermann. The clean data/codata
  introduction in the Fun→Core compilation. Cleanest first
  read.
