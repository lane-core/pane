---
type: analysis
status: current
audited_by: [session-type-consultant@2026-04-11]
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [cbpv, f_u_adjunction, returner, thunk, levy, value_computation, upshift, downshift, handles, message, reply_port]
related: [analysis/foundations/_hub, analysis/duploid/polarity_and_composition, analysis/duploid/writer_monad_and_optics, analysis/foundations/data_vs_codata, reference/papers/grokking_sequent_calculus]
sources: [../psh/.serena/memories/analysis/cbpv_f_u_separation]
verified_against: [../psh/docs/specification.md@2026-04-11]
agents: [session-type-consultant, optics-theorist, pane-architect]
---

# CBPV F/U separation (call-by-push-value)

## Problem

Pane has types that clearly exist (`Message`, `ServiceFrame`,
`Address`) and types that clearly happen (`Handles<P>`,
`HandlesRequest<P>`, `Handler` lifecycle). The split isn't
cosmetic — it's Levy's call-by-push-value adjunction surfaced
as Rust type structure. Naming the adjunction makes the
boundary operators (`ReplyPort` as shift, `ActivePhase<T>` as
the handshake-to-active transition) visible as instances of a
single theoretical pattern.

## Resolution

Levy's **Call-by-Push-Value** splits types into two kinds:

- **Value types** (positive, A): things that exist, can be
  stored, can be substituted. Pane instances: `Message`,
  `ServiceFrame`, `Address`, `ControlMessage`.
- **Computation types** (negative, B): things that happen, can
  be sequenced, can produce values. Pane instances:
  `Handles<P>`, `HandlesRequest<P>`, the `Handler` lifecycle
  surface.

Two adjunction operators bridge them:

- **`F : Val → Comp`** — the **returner** / **upshift** ↑.
  Given a value type A, `F(A)` is "a computation that returns
  a value of type A." In pane, a `Handles<P>` method that
  reads state and returns a `Flow` is F-typed: the observation
  side of a computation.
- **`U : Comp → Val`** — the **thunk** / **downshift** ↓.
  Given a computation type B, `U(B)` is "a value that
  suspends a computation of type B." In pane, `ReplyPort<T>`
  is the load-bearing U-site: a *value* (move-only, passed
  in `Request` variants of `ServiceFrame`) that, when
  consumed via `.reply()`, forces the suspended computation
  (firing the installed `DispatchEntry`). Dropping a
  `ReplyPort` is the Drop-as-force path ([pane-local gloss];
  not from Levy or psh) that compensates by firing the
  installed callback with `Err(ReplyFailed)` — see
  `crates/pane-proto/src/obligation.rs` for the `ReplyFailed`
  unit struct and Drop impl. `ActivePhase<T>` (still a design stub per
  `status`) is the positive residue of the handshake dialogue
  — another U-shaped value carrying the computation's
  negotiated state.

Preserved caveats from psh's original anchor: **Levy's CBPV
monograph is not vendored locally**. psh's canonical source
for the CBPV framework is `docs/specification.md` lines
218–230 plus its `decision/let_is_mu_tilde_binder_cbpv` memo;
pane inherits the same sourcing. The operational consequence
psh highlights is that `let x = effectful_call` is the μ̃-binder
on monadic bind — pane's shell-integration work (phase 2+)
will want to preserve this.

## Status

Adopted from psh 2026-04-11; treat as design vocabulary for
pane. Pane's `ReplyPort` is the clearest F/U instance in the
current codebase; future shell-integration work will surface
the syntactic split explicitly. Not formally verified that
`ReplyPort`'s Drop-as-force semantics fully realize U(B) in
the Levy sense — the structural correspondence is what the
anchor consumes.

## Source

- `../psh/docs/specification.md` §"Theoretical framework §The
  practice" lines 218–230 — Levy CBPV citation. psh's canonical
  anchor.
- `../psh/docs/specification.md` §"Two kinds of callable" line
  473 — `def` vs lambda as F/U.
- `reference/papers/grokking_sequent_calculus` — Binder et al.,
  CBPV-flavored Fun→Core compilation. Accessible introduction;
  read this first if F/U is new.
- Levy, P.B. *Call-by-Push-Value: A Functional/Imperative
  Synthesis.* Springer 2004. **Not vendored** in `~/gist/`.
