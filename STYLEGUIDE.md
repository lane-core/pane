# Style Guide

Conventions for contributing to pane — code, prose, and formatting.
Observed by all contributors, human and agent alike.

For naming conventions, see [`docs/naming-conventions.md`](docs/naming-conventions.md).
For API documentation style, see [`docs/kit-documentation-style.md`](docs/kit-documentation-style.md).

---

## Rust Formatting

| Rule | Setting |
|------|---------|
| Formatter | `cargo fmt` via `rustfmt.toml` |
| Max line width | 100 |
| Import granularity | Per-crate (`use pane_proto::{Flow, ServiceFrame}`) |
| Import grouping | std, then external crates, then local (`group_imports = "StdExternalCrate"`) |
| Linter | `cargo clippy -- -D warnings` |

Run `cargo fmt` before every commit. Run `cargo clippy --workspace`
before pushing. Fix warnings — don't suppress them.

The `rustfmt.toml` is the source of truth. No editor-local overrides.

## Derive Order

When a type derives multiple traits, use this order:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
```

Only derive what's needed. Omit `Copy` unless the type is small and
value-semantic. Omit `Serialize`/`Deserialize` unless the type
crosses a wire boundary.

## Visibility

| Scope | Annotation |
|-------|------------|
| Public API | `pub` |
| Crate-internal | `pub(crate)` |
| Test support exposed to integration tests | `#[doc(hidden)] pub` |
| Everything else | private (no annotation) |

## Error Handling

| Context | Pattern |
|---------|---------|
| Public API boundaries | `Result<T, E>` — never panic |
| Internal invariants | `assert!` with a message |
| Deserialization of untrusted wire data | `match` on `Result`, handle `Err` gracefully |
| Session types / obligation handles | Drop compensation (send failure signal on drop) |
| Handler callbacks | `catch_unwind` boundary at the looper |

Three error channels, by design:
- **Protocol**: `ReplyPort` / `CompletionReplyPort` (typed reply or `ReplyFailed`)
- **Control**: `Flow::Stop` (handler requests exit)
- **Crash**: `panic` caught by `catch_unwind` → destruction sequence

---

## Code Comments

Comments explain *why*, not *what*. If the code needs a *what*
comment, the code needs rewriting.

```rust
// Bad: increment the counter
count += 1;

// Good: session_ids are monotonic per-connection; 0 is reserved for control
let session_id = self.next_session;
self.next_session += 1;
```

Do not add comments to code you didn't change. Do not add
docstrings to private functions unless the logic is genuinely
non-obvious.

### Heritage Annotations

Every module and significant type documents its design lineage
from BeOS/Haiku and Plan 9 where applicable.

**Module-level** — `//! Design heritage:` block in the module doc:

```rust
//! Design heritage: BeOS BLooper::task_looper()
//! (src/kits/app/Looper.cpp:1162) blocked via MessageFromPort().
//! Plan 9 devmnt gated callers as readers one at a time per mount
//! (devmnt.c mountio():803).
```

**Type/trait-level** — inline in the doc comment:

```rust
/// Plan 9: analogous to qid.path (stable, machine-comparable)
/// BeOS: team_id was kernel-assigned but self-reported
```

**Every heritage claim cites a source.** No source, no claim.

### Reference repositories

| Source | Location | Pinned at |
|--------|----------|-----------|
| Haiku | `/Users/lane/src/haiku` (github: `haiku/haiku`) | `e47c047de4` |
| Plan 9 | `reference/plan9/` vendored (github: `plan9foundation/plan9`) | `ff579efb6d` |

### Citation format

| Tradition | Format |
|-----------|--------|
| Be/Haiku | `src/kits/app/Looper.cpp:1162` (path relative to Haiku root) |
| Plan 9 | `reference/plan9/man/5/0intro:91-96` or `reference/plan9/src/sys/src/9/port/devmnt.c:803` |
| Papers | Author, title, year, section (e.g., `MMM25, Mangel/Melliès/Munch-Maccagnoni 2025`) |

If the Haiku source moves (rebase, refactor), update the pinned
hash here and re-verify citations. Plan 9 source is vendored and
stable — citations are relative to `reference/plan9/` in this repo.

If a citation can't be found, soften the claim ("similar to"
rather than "from") and flag for follow-up verification.

### Theoretical Annotations

When code implements a concept from the duploid framework,
session-type theory, or optics, cite the source concisely using
the bibliography key form (see Citation Standard below):

```rust
//! Theoretical basis: the active phase is a plain (non-dialogue)
//! duploid ([MMM25]). Sequential dispatch prevents non-associative
//! cross-polarity composition.
```

Keep theoretical annotations brief. One sentence identifying the
concept and its source. The reader who needs the full treatment
will find it in the serena memories or cited papers. See
Citation Standard below for the full discipline including the
authoritative bibliography at `docs/citations.md`.

---

## Code Documentation and Citation Standard

A documentation standard for pane's code-level citations. The
bibliography is in [`docs/citations.md`](docs/citations.md) and
is the only authoritative source for citation keys. Every
`[Key]` token in pane's `//!` / `///` doc comments must resolve
to an entry there.

### Principles

1. **The module is the primary documentation unit.** Module-level
   docs (`//!`) establish the conceptual frame. Function-level
   docs (`///`) fill in detail within that frame.
2. **Document the role before the mechanism.** What a module or
   function is *for* comes before how it works. The reader needs
   to know where they are in the architecture before they can
   understand the code.
3. **Cite what you drew from.** If a reference informed the
   design of a specific construct, cite it. If it informed the
   broader architecture, cite it at the module level. If it's
   background knowledge, it belongs in the bibliography, not in
   the code.
4. **Credit authors by name.** Inline citations use the
   bibliography key (e.g., `[JHK24]`), and the reader can
   resolve the key via `docs/citations.md` without leaving the
   repo. The bibliography entry carries the full author list.
5. **Citations follow the implementation.** When code moves,
   citations move with it. When code is deleted, citations are
   deleted. A citation is an attribute of a specific
   implementation, not a permanent annotation on a concept.

### Module-Level Documentation (`//!`)

Every crate entry point and every significant module gets a
module-level doc comment. The module doc establishes:

- **What this module is.** Its role in the pane architecture,
  stated in the vocabulary `docs/architecture.md` uses. If the
  module belongs to a specific layer or phase (Phase 1 vocabulary,
  active phase, handshake phase), name which.
- **What design documents it implements.** Direct references to
  spec sections (`docs/architecture.md` §Name) or decision memos
  (`decision/<topic>` in serena).
- **What references informed it.** The theoretical or external
  sources that shaped the module's architecture.
- **A `# References` section.** A scoped bibliography at the end
  of the module doc, listing citation keys from `docs/citations.md`.
  This gives the reader a focused reading list.

**Template:**

```rust
//! # pane-app Dispatch — actor-level request/reply and notification routing
//!
//! Dispatch owns the per-request token table and the service
//! dispatch table. It is the runtime site where the EAct rules
//! E-Suspend (install a handler into the dispatch store) and
//! E-React (fire an installed handler on reply arrival) land.
//!
//! The module belongs to the active-phase runtime: handshake is
//! already complete, `par`-typed session channels have been
//! consumed, and the remaining work is one-shot request/reply
//! flows routed by `(ConnectionId, Token)` keys. See
//! `docs/architecture.md` §Dispatch and `decision/server_actor_model`
//! for why the dispatch layer is single-threaded.
//!
//! # References
//!
//! - `[FH]` §3.2 — E-Suspend / E-React rules. The install-then-fire
//!   shape is a defunctionalized Rust realization.
//! - `[JHK24]` §1 — affine-plus-closure-capability encoding of
//!   linearity. `ReplyPort`'s Drop compensation plays the role of
//!   the paper's unforgeable `End` token.
//! - `[CMS]` §5.1 — forwarder cut-admissibility. The dispatch
//!   layer forwards at runtime; chains of dispatch entries across
//!   multiple `ServiceHandle<P>` invocations are cut-chain
//!   compositions in the operational sense.
```

### Function-Level Documentation (`///`)

Function docs are secondary to the module doc. They fill in
specifics within the frame the module doc establishes.

**Lead with the role.** "Installs a one-shot handler keyed on
`(connection, token)`" before "locks the HashMap and inserts."
The reader needs the *what* before the *how*.

**State preconditions and postconditions** when they are not
obvious from the type signature. What the function expects,
what it guarantees, what it may fail with.

**Cite only when this specific function draws from a reference.**
The threshold question: did we draw on this reference to
construct *this function's* implementation? If the reference
informed the module's architecture but not this function
specifically, the module doc handles it. Don't repeat
module-level citations at the function level unless the
function has its own specific dependence.

**Inline citation form:**

```rust
/// Installs a one-shot reply continuation keyed on the request
/// token. Consumed by [`fire_reply`] or [`fire_failed`]; the
/// entry is removed on fire, realizing the one-shot semantics
/// of EAct's E-React rule.
///
/// Reference: [FH] §3.2 (E-Suspend / E-React).
pub fn insert(...) { ... }
```

The key appears inline next to the claim it supports. Full
bibliographic details are one lookup away in `docs/citations.md`.

### What not to document

- **Don't narrate the code.** If the implementation is clear
  from reading it, don't restate it in prose. Document *why*,
  not *what*. (This matches the general Code Comments rule
  above.)
- **Don't document private helpers** unless their behavior is
  surprising or their invariants are non-obvious. The module doc
  covers the module's internal structure at a high level; not
  every private function needs its own doc comment.
- **Don't cite references you didn't draw from.** A citation is
  a claim: "this reference informed this implementation." If
  you're citing for background or prestige, put it in the
  bibliography's annotation, not in the code.

### Citation hygiene

Citations are implementation attributes. They require
maintenance.

- **When refactoring a function:** check whether its citations
  still apply. If the function no longer implements the cited
  construct, remove or update the citation. A stale citation is
  worse than no citation — it actively misleads.
- **When restructuring a module:** update the module-level
  `# References` section. Add references the module now draws
  from; remove ones it no longer needs.
- **When adding a new module or function:** check
  `docs/citations.md` for relevant references before writing
  docs. If your implementation draws on a reference, cite it.
  If you are unsure, note `// TODO: citation needed` and flag
  for formal-verifier review.
- **When retiring a module or function:** citations are deleted
  with the code. They don't need to be preserved elsewhere.

### Mechanical vs semantic audit

Pane ships a linter that checks citation resolution:

```
just cite-lint
```

`cite-lint` catches:

- **Typos.** `[JKH24]` instead of `[JHK24]`.
- **Renamed or removed keys.** A citation whose bibliography
  entry no longer exists.
- **Unused bibliography entries.** Keys in `docs/citations.md`
  that no `//!` / `///` comment cites.
- **Alias conflicts.** Two entries claiming the same alias.

`cite-lint` does NOT catch:

- **Stale citations on refactored functions.** When a function's
  implementation changes but the citation stays, the citation
  may no longer reflect what the code does. The tool sees the
  citation; it cannot see whether it is accurate.
- **Wrong paper attribution.** A citation that resolves
  syntactically but cites the wrong paper for the implementation
  claim.
- **Epistemic strength violations.** A citation whose inline
  phrasing strengthens the source beyond what the source
  actually says (see `policy/memory_discipline` §10 —
  Principle 10, Epistemic strength matches the source).
- **Refactor-induced drift** of any kind that preserves
  syntactic validity.

**The only way a citation is verified is by a human or
formal-verifier re-reading the cited reference against the
code and the implementation claim.** `cite-lint` is a floor —
it rules out obvious failures so reviewers spend their
attention on the substantive ones. A green `cite-lint` is
necessary but not sufficient. Treat it as `clippy` for
citations: syntactic hygiene, not semantic correctness.

The semantic audit is the formal-verifier's job and runs as
part of the tier-2 audit procedure in `policy/agent_workflow`
(which also enforces Principle 10 on memory paraphrases). See
`policy/refactor_review_policy` for the two-step discipline
required on any refactor that touches cited functions:

1. Run `just cite-lint` to catch trivial breakage.
2. Re-read the cited references and confirm the surviving
   citations still describe the code accurately. Leave a
   reviewer comment confirming the check — "citations
   re-verified against [JHK24] §1 and [FH] §3.2."

Green CI cannot substitute for the second step.

### Ethical citation practices

The purpose of citing in code documentation is threefold:

1. **Credit.** The authors whose ideas informed the
   implementation deserve attribution. Name them (via the
   bibliography entry that the key resolves to).
2. **Traceability.** A future reader can follow the citation to
   understand *why* a construct is designed the way it is. The
   reference provides the justification; the code provides the
   realization.
3. **Verifiability.** An auditor can check whether the
   implementation faithfully realizes the cited construct. The
   citation is a testable claim: "this code implements the idea
   described in this reference." If the code diverges from the
   reference, the citation should note the divergence
   (typically via a "but see" hedge in the doc comment or a
   decision memo link).

Do not cite to impress. Do not cite references you have not
read. Do not cite references that did not inform the specific
implementation being documented. Every citation is a claim of
intellectual debt — make it honestly.

---

## Technical Writing Voice

Describe the machine. Present tense, active voice, concrete behavior.
Trust the reader to infer concepts from behavior.

Short sentences. Code examples over prose. If a formalism name is
needed, use it once briefly — do not lead with it or justify why
before explaining what.

```
Bad:  "pane-app provides a single-threaded actor dispatch
       framework based on the EAct model."

Good: "Each pane runs on one thread. The looper dispatches
       messages sequentially."
```

| Guideline | Example |
|-----------|---------|
| Present tense | "The looper reads from the channel" not "will read" |
| Active voice | "The reader thread posts events" not "events are posted" |
| State consequences | "Panics if the channel is closed" not "may fail" |
| Second person in API docs | "Use this to..." not "This can be used to..." |
| No hedging | "does" not "may", "fails" not "might fail" |

### Agent Identity in Examples

Use generic human names for agent identities in docs and
examples: `ada`, `bob`, `ralph`. Not dotted names like
`agent.reviewer` — dotted names are invalid unix usernames,
and pane maps certificate subjects to local accounts.

---

## Testing

| Rule | Detail |
|------|--------|
| Framework | `#[test]` with `cargo test --workspace` |
| Property tests | `proptest` for serialization roundtrips |
| Test environment | macOS, in-memory channels (`MemoryTransport`) |
| Integration tests | `crates/*/tests/` — prove cross-crate wiring |
| Test naming | `snake_case` describing the claim being tested |

Test names should read as claims:

```rust
#[test]
fn drop_sends_revoke_interest() { ... }

#[test]
fn declare_interest_no_provider_declined() { ... }

#[test]
fn obligation_handles_fire_drop_on_destruction() { ... }
```

---

## Architecture Patterns

These are not just conventions — they are load-bearing design
decisions documented in `docs/architecture.md`.

| Pattern | Rule |
|---------|------|
| Threading | One thread per pane (BLooper model). Sequential dispatch. |
| Two-phase connection | Phase 1 verifies transport (`Result`). Phase 2 runs handshake. |
| Handler returns `Flow` | Not `Result`. Three error channels (protocol, control, crash). |
| Obligation handles | `#[must_use]`, Drop compensation. `ReplyPort`, `CancelHandle`. |
| Functoriality | Phase 1 types are the full architecture's types, populated minimally. |
| No stability commitment | Remove dead code outright. Don't deprecate, don't `#[allow(dead_code)]`. |
| Ghost state discipline | Typed ownership over correlation IDs at API surfaces. |

---

## Divergence Protocol

When deviating from a Be name or Plan 9 pattern:

1. Record the divergence in the appropriate serena memory
   (`pane/beapi_divergences` or `pane/plan9_divergences`)
2. Add a `# BeOS` or `# Plan 9` section to the doc comment
3. Valid reasons: architecturally different, Rust idiom supersedes,
   established contemporary convention
4. Invalid reasons: "sounds better", "more modern", didn't check
   what Be/Plan 9 called it
