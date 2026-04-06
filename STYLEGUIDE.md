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
session type theory, or optics, cite the source concisely:

```rust
//! Theoretical basis: the active phase is a plain (non-dialogue)
//! duploid (MMM25). Sequential dispatch prevents non-associative
//! cross-polarity composition.
```

Keep theoretical annotations brief. One sentence identifying the
concept and its source. The reader who needs the full treatment
will find it in the serena memories or cited papers.

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
