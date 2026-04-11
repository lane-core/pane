# Shell as Sequent Calculus: Analysis Summary (2026-04-05)

Lane explored grounding a pane shell's semantics in the linear classical L-calculus (Mangel, Mellies, Munch-Maccagnoni 2025) and its connection to session types via dialogue duploids. Eight agent consultations (4 initial + 4 cross-deliberation) reached consensus.

## Verdict: compelling, Phase 2+ work

A shell using pane-proto, pane-app, pane-session as sole dependencies is a natural component of "pane linux." The theoretical grounding provides genuine structural insight. Designing it now is valuable; building it now would be premature.

## Architecture (revised 2026-04-05, standalone-first)

**The shell is two things:**

1. **pane-shell** — standalone binary, login-shell capable. Deps: pane-proto + pane-session (libraries only). Own input loop, signal handling, fork/exec/wait, job control. `get`/`set` optional — connect to pane servers when available, error gracefully when not.

2. **pane-terminal** — LooperCore<TerminalHandler> hosting wrapper. Only for pane compositor. VT parsing, PTY bridge, MonadicLens attrs.

Rationale: rc never imported rio headers. The interpreter should not need LooperCore.

### Session type verdict on standalone-first (2026-04-05)

**Conditionally sound.** Session types are genuinely optional. Shell_standalone subset of Shell_pane. Duploid structure (layers 1-2) fully operative in standalone. Session types (layer 3) add `get`/`set` capabilities, not behavioral changes. No split personality.

**Open question:** Do `get`/`set` use pane-fs (filesystem, no session needed) or speak protocol directly (session-typed, cached connection with reconnection)? If filesystem: pane-session unused by shell. If protocol: affine gap compensated by transport EOF.

**Job table ruling:** NOT obligation handles. Jobs are unilateral multi-op resources. Command substitution's fork/wait IS an obligation handle use case.

## Three-layer formalism (all agents agree on boundaries)

| Layer | Formalism | Governs |
|-------|-----------|---------|
| Pipeline AST | Sequent calculus | Cuts, fd linearity, redirection as continuation substitution |
| State access | Profunctor optics | Namespace paths as AffineFold composition, tab-completion as traversal enum |
| IPC transport | Session types | Shell↔server handshake, obligation handles, service binding |

**Critical rule:** Session types govern vertical (shell↔server). Optics govern state projection. Sequent calculus governs execution model. Pipes carry bytes — don't session-type them.

## Genuine structural correspondences

- **Values** = data (file contents, strings). Positive, eager.
- **Stacks** = file descriptors / continuations / consumers. Negative.
- **Commands** = pipeline stages as cuts ⟨producer | consumer⟩.
- **Redirection = continuation manipulation.** Also has profunctor structure: `>` is lmap (contravariant output subst), `<` is rmap (covariant input subst), `2>&1` is contraction on contravariant component.
- **CBV/CBN coexistence** — variable expansion is eager (CBV/positive), pipeline stages lazy/demand-driven (CBN/negative).
- **B5 (pure reads) keeps duploid associative.** Also ensures idempotency and prevents self-deadlock when shell reads own attributes.
- **Tab-completion = traversal enumeration** at namespace prefix.
- **Same-pane pipelines are profunctor optic modifications.** Cross-pane pipelines are spans.

## What doesn't hold up

- **Centrality = thunkability** — true but not actionable for shell design. Useful only for a future pipeline optimizer.
- **Making calculus visible in syntax** — unanimous rejection. Syntax should be rc-clean.
- **Session-typing pipes** — overengineering. Pipes carry bytes.
- **Duploid non-associativity for pipeline grouping** — models a distinction the shell syntax doesn't expose and the user never controls.

## Unresolved design questions (need Lane's call)

1. **Protocol-first vs filesystem-first** — interpreter speaks protocol (works without FUSE); terminal pane composes with filesystem tools. Session-type consultant analysis (2026-04-05): if filesystem, pane-session is unused by shell; if protocol, session types govern `get`/`set` connection lifecycle. Priority ordering TBD.
2. **`do` as third builtin** — session type agent recommends against; `ctl_fallback` is sufficient. Risk of BeOS-style scripting divergence.
3. **Jobs as MonadicLens** — Plan 9 agent: read-only namespace state (like /proc), not a lens. Optics agent: process table is Traversal, not Lens.
4. **Profunctor reading of redirection** — document formally or leave as theoretical background?

## Papers referenced

- Mangel, Mellies, Munch-Maccagnoni (2025): dialogue duploids, L-calculus, non-associative composition
- Munch-Maccagnoni (2014b): original duploid definition
- Wadler (2012): "Propositions as Sessions" — cut as parallel composition
- Clarke et al.: mixed optics, MonadicLens definition, profunctor representation theorem

## Related psh concept anchors

psh's `.serena/memories/analysis/` layer maintains keyword-
shaped concept anchors that this memo's Layer 1 (sequent
calculus) consumes at a tier-1 level of detail. For theoretical
depth on specific concepts invoked above:

- **`../psh/.serena/memories/analysis/three_sorts`** — the
  three sorts (values/producers Γ, stacks/consumers Δ,
  commands/cuts ⟨t|e⟩) as the syntactic basis of the sequent
  calculus. psh's AST extends this to four sorts (with Mode);
  pane's shell-model abstraction level is Lane's call.
- **`../psh/.serena/memories/analysis/cut_as_execution`** —
  `⟨t|e⟩` as the execution rule, with the full catalog of
  instances (pipes, assignments, conditionals, match). psh's
  spec lines 309–315 table is the detailed source.

Not vendored into pane. Cited here for retrieval; the merge
test (`policy/memory_discipline` §7) keeps them separate from
this spoke's Phase 2+ pane-shell design narrative.
