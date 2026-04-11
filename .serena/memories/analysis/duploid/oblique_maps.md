---
type: analysis
status: current
audited_by: [optics-theorist@2026-04-11]
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [oblique_map, cross_polarity, p_to_n, producer_consumer, boundary, duploid, shift, frame, reply_port, service_handle]
related: [analysis/duploid/_hub, analysis/duploid/polarity_and_composition, analysis/foundations/decision_procedure, reference/papers/duploids]
sources: [../psh/.serena/memories/analysis/oblique_maps]
verified_against: [../psh/docs/vdc-framework.md@2026-04-11, /Users/lane/gist/classical-notions-of-computation-duploids.gist.txt@HEAD]
agents: [optics-theorist, formal-verifier]
---

# Oblique maps (cross-polarity arrows P → N)

## Problem

Pane's IPC boundary — wire frames meet handler dispatch — is
neither a pure value-composition nor a pure context-composition
operation. It lives in the gap between the two subcategories
and deserves a name. Without one, the boundary looks like an
ad-hoc implementation detail rather than an instance of a
structural pattern that also shows up in shells (psh), in
classical Be APIs, and in Plan 9's 9P.

## Resolution

In a duploid, **oblique maps** are arrows from a positive
object to a negative object: `P → N`. They are neither
pure-monadic (Kleisli, P → P) nor pure-comonadic (co-Kleisli,
N → N) — they live in the gap between the two subcategories.
Psh's reading (ported here) is that **every shell command has
the structure of an oblique map**: a value-mode argument list
(positive, after expansion) met by a computation-mode effect
(negative, process execution).

In pane, the same structural pattern appears at several
boundaries:

- **`ServiceHandle::send_request`.** A positive `P::Message`
  value meets a negative handler expecting to receive it.
  Install-before-wire (`architecture/app`) serializes the
  crossing; the returned `ReplyPort` is the shift operator
  wrapping the negative continuation as a positive value for
  later forcing.
- **Ctl writes.** A positive byte string meets a negative
  handler responding via a `MonadicLens` set operation. See
  `analysis/optics/panefs_taxonomy` for the per-entry
  classification — ctl entries are **not** optics; they are
  the oblique boundary.
- **`AttrWriter::write`.** A positive value applied to a
  negative state observer, producing a `Vec<Effect>` writer-
  monad residue (`analysis/optics/writer_monad`). The effectful
  set is the oblique map realization.
- **`Handler::pane_exited` dispatch.** A positive
  `ControlMessage::PaneExited` value meets a negative handler
  observer. Single-mailbox delivery via the actor
  (`decision/server_actor_model`) is what keeps the
  (+,−) non-associativity from manifesting as a concurrent
  bracket.

Whether the full duploid composition laws carry over from
psh's shell setting to pane's actor / filesystem setting is
**not formally verified** — the structural correspondence is
what the anchor consumes. The important operational
consequence, per the (+,−) non-associativity
(`analysis/duploid/polarity_and_composition`), is that every
oblique map is a boundary crossing and every boundary crossing
needs a frame. Pane's single-threaded actor is the runtime
enforcement: the frame discipline is implicit in the type
system because the looper serializes all polarity crossings.

The Mangel-Melliès-Munch-Maccagnoni **locally vendored** PACMPL
paper at `/Users/lane/gist/classical-notions-of-computation-duploids.gist.txt`
covers the (+,−) non-associativity proof (lines 7100–7185)
but does **not** contain the "oblique map" terminology
directly — that vocabulary is from the companion FoSSaCS 2014
paper, which is not vendored. Psh's anchor reports the same
gap and cites `refs/ksh93/ksh93-analysis.md` §"Monadic and
comonadic patterns in C" line 459 as its proximate source
within psh's own materials. Pane inherits the same sourcing
caveat.

## Status

Adopted from psh 2026-04-11; use "oblique map" as design
vocabulary for producer-consumer boundary operations. When
explaining pane's IPC boundary in one sentence, "every
producer-consumer crossing has the structure of an oblique
map, enforced by the actor model" is the port. Not formally
verified that the full duploid composition laws apply — treat
as structural analogy grounded in the vendored duploids paper's
non-associativity proof.

## Source

- `reference/papers/duploids` —
  Mangel-Melliès-Munch-Maccagnoni. Vendored at
  `/Users/lane/gist/classical-notions-of-computation-duploids.gist.txt`.
  Covers the (+,−) non-associativity proof (lines 7100–7185).
  Does not contain Table 1 or direct "oblique" treatment —
  that is in the un-vendored FoSSaCS 2014 companion.
- `../psh/refs/ksh93/ksh93-analysis.md` §"Monadic and
  comonadic patterns in C" line 459 — psh's proximate source
  for the Table 1 mapping. Not vendored into pane.
- `../psh/docs/vdc-framework.md` §5.4 "Cells = Commands" line
  443 — psh's framing of commands as cells aligned with
  oblique maps.
- `analysis/duploid/polarity_and_composition` — the (+,−)
  non-associativity result and pane's concrete polarity
  assignments.
- `decision/server_actor_model` — single-mailbox actor as the
  runtime enforcement of the polarity frame discipline.
