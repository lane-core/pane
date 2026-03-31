# Kit Documentation Style Guide

How to write API documentation for pane's kit crates. Derived from
the Be Book and Haiku Book conventions, adapted to Rust doc comments.

This is a process document — follow it when implementing new
subsystems, adding public API surface, or reviewing documentation.

---

## Principles

1. **The developer shouldn't need the newsletter.** Everything the
   Be Book left to newsletter articles, we put in the docs. Design
   rationale belongs in `//!` overviews, not in external documents.

2. **Heritage is context, not identity.** Noting the Be/Haiku
   ancestor helps developers find their footing and understand why
   the API has its shape. But pane's docs stand alone — a developer
   who has never heard of BeOS should understand every type and method
   from the doc comment alone.

3. **Match Be's tone, not their format.** Second-person, practical,
   consequence-stating voice. Not Doxygen tag structure.

4. **Rust doc conventions are load-bearing.** `# Examples`,
   `# Panics`, `# Errors`, `# Safety` are expected by Rust
   developers. Use them.

5. **Document the contract, not the implementation.** What the method
   promises, what it requires, what it does NOT do.

---

## Attribution Policy

Be, Inc. designed the original API. The Haiku project spent 25 years
extending, refining, and documenting it. Both contributions are
invaluable and both are credited.

- Heritage annotations should name the Be ancestor and reference
  Haiku's documentation where Haiku extended or clarified the
  original design.
- The Haiku Book lives at `reference/haiku-book/` in this repo.
  Link to specific `.dox` files when they informed a design choice.
- Use plain backtick syntax for Be/Haiku class names: `` `BHandler` ``,
  not `` [`BHandler`] ``. The bracket form triggers rustdoc's
  broken-link warnings since these types don't exist in pane's
  crate graph. Plain backticks render identically in the output.

---

## Crate-Level `//!` (Kit Overview)

Analogous to the Be Book's kit introduction page.

**Must contain:**
- What this crate is, in one paragraph
- The BeOS/Haiku kit it descends from, noted parenthetically
- What problem it solves (not what it contains)
- A `# Quick Start` example showing the minimal complete program
- Brief roadmap of the public types, grouped by role (not alphabetically)
- Cross-references to related crates

**Must NOT contain:**
- Exhaustive type listings (that's what the module pages are for)
- Internal architecture details
- Historical narrative about BeOS (save that for type-level docs)

**Template:**

```rust
//! {One-sentence purpose statement.}
//!
//! {What problem this solves and for whom. 2-3 sentences max. Mention
//! the BeOS/Haiku ancestor kit parenthetically if it illuminates the
//! design.}
//!
//! # Quick Start
//!
//! ```ignore
//! // Minimal complete example, 10-20 lines
//! ```
//!
//! # Types
//!
//! The kit provides:
//!
//! - **[`TypeA`]** — {role in one clause}
//! - **[`TypeB`]** — {role}
//! ...
//!
//! # Related Crates
//!
//! - [`pane_proto`] — {relationship}
```

---

## Type-Level `///` (Struct, Enum, Trait)

Analogous to the Be Book's class description.

**Structure:**

1. **Brief**: one sentence. What this type *is*.
2. **Overview**: 1-4 paragraphs covering:
   - What problem this type solves
   - How it fits into the larger picture (relationships to other types)
   - The ownership/lifecycle model (consumed by run? cloneable? Send?)
   - Threading considerations, stated explicitly
3. **`# Threading`** section when threading semantics are non-trivial
4. **`# BeOS`** section last, when applicable (see Heritage Annotations below)

**Tone:**
- Second person: "Use this to...", "You can...", "If your handler needs to..."
- State consequences of misuse directly
- When explaining a type relationship, explain *why* it exists

**How verbose?**
- Core types (App, Pane, Handler, Messenger, Message): 3-4 paragraphs minimum
- Supporting types (Tag, FilterChain, TimerToken): 1-2 paragraphs
- Wire types re-exported from pane-proto: 1 sentence + link to pane-proto doc

---

## Method-Level `///`

Analogous to the Be Book's method documentation.

**Structure:**

1. **Brief**: one sentence, start with a verb.
2. **Body** (when needed): when to use it, preconditions, side effects,
   return value semantics.
3. **Standard sections** (as applicable):
   - `# Examples` — when the usage has subtlety (not for self-evident getters)
   - `# Panics` — if it can panic (and when)
   - `# Errors` — for Result-returning methods, enumerate the error variants
4. **`# BeOS`** — only if the method's name or semantics diverge notably

**Verbosity guidelines:**
- **Getters/setters with obvious semantics**: brief only.
- **Lifecycle methods** (connect, create_pane, run, run_with): full treatment.
  These are the methods developers get wrong.
- **Hook methods** (Handler trait): explain what triggers the call, what
  the default does, what returning Ok(false) means, and give a concrete
  use case.
- **Side-effecting methods** (set_title, send_message, monitor): state what
  happens, when it takes effect, and what can go wrong.

**Hook method pattern (Handler trait):**

```rust
/// Called when the pane gains focus.
///
/// The compositor sends this when the user clicks on or tabs to
/// this pane. Override to update visual state (cursor style,
/// selection highlight) or start input capture.
///
/// Default: continues the event loop (`Ok(true)`).
///
/// # BeOS
///
/// `BWindow::WindowActivated` — pane splits the `bool active`
/// parameter into separate `activated` / `deactivated` hooks.
fn activated(&mut self, _proxy: &Messenger) -> Result<bool> {
    Ok(true)
}
```

---

## Field-Level `///`

**When to document:**
- Public fields with non-obvious semantics
- Fields whose units or valid ranges aren't encoded in the type
- Enum variant fields when the name doesn't tell the full story

**When to skip or keep minimal:**
- Fields whose name and type make the doc redundant

**Rule:** if a field has a valid range, unit, or constraint not
captured by the type, document it. Otherwise a one-line phrase.

---

## Enum Variant Documentation

For the `Message` enum (and similar dispatch enums), each variant needs:
- When it's delivered (what triggers it)
- Which Handler trait method it dispatches to
- `# BeOS` only when the mapping is non-obvious

```rust
/// The pane is ready and its initial geometry is known.
///
/// Always the first event delivered. Dispatches to
/// [`Handler::ready`].
Ready(PaneGeometry),

/// The pane was resized by the compositor or layout engine.
///
/// Dispatches to [`Handler::resized`].
///
/// # BeOS
///
/// `B_WINDOW_RESIZED` / `BWindow::FrameResized`.
Resize(PaneGeometry),
```

---

## Heritage Annotations: `# BeOS` and `# Plan 9`

Heritage sections are placed *last* in the doc comment, after all
functional documentation. Their purpose is context — helping
developers understand why the API has its shape by tracing its
lineage to Be and Plan 9.

pane draws from two traditions:
- **BeOS/Haiku** — the application kit (threading model, handler
  pattern, messenger, filter chain, timer model)
- **Plan 9/Inferno** — the distributed architecture (per-process
  namespaces, synthetic filesystems, 9P-inspired protocols,
  location-transparent connectivity, clunk-on-abandon)

A type or method may have annotations from one or both traditions.

### `# BeOS` format

**On types with clear ancestry** (list *changes*, not similarities):

```rust
/// # BeOS
///
/// Descends from `BHandler` (see also Haiku's
/// [BHandler documentation](reference/haiku-book/app/BHandler.dox)).
/// Key changes:
/// - Trait with default methods replaces virtual class
/// - Returns `Result<bool>` instead of `void`
/// - Per-event methods replace single `MessageReceived(BMessage*)`
```

**On types with trivial mapping** (one line):

```rust
/// # BeOS
///
/// `BMessenger`.
```

### `# Plan 9` format

**On types/methods with Plan 9 lineage** (cite the specific concept):

```rust
/// # Plan 9
///
/// `import` — connecting to a remote server and using it as if
/// local. The local machine has no architectural privilege; a
/// remote server is just another server with higher latency.
```

**On types bridging both traditions:**

```rust
/// # BeOS
///
/// `BApplication` — but not a looper. Per-pane loops replace
/// the application-level loop.
///
/// # Plan 9
///
/// `connect_remote` is the `import` equivalent — mounting a
/// remote pane server into the local application's namespace.
```

### When to use each

| Lineage | Use `# BeOS` | Use `# Plan 9` |
|---------|-------------|----------------|
| Type/method naming | When name derives from Be | (Plan 9 didn't influence naming) |
| Threading model | Looper, Handler, per-pane threads | Per-process event loop model |
| Distribution | (Be was single-machine) | Remote connectivity, namespace, identity |
| Protocol | (Be used kernel ports) | 9P patterns: clunk, walk, stateful sessions |
| Filesystem | (Be had BFS attributes) | Synthetic filesystem, ctl files, union dirs |

### Rules

1. Heritage sections are always last, after `# Examples`, `# Panics`,
   `# Errors`, `# Safety`
2. `# BeOS` before `# Plan 9` when both appear (Be came first historically)
3. List *changes*, not similarities
4. When a pane concept is a genuine novelty (command surface, tag),
   don't force a heritage mapping
5. Reference the Haiku Book when Haiku's documentation informed the
   design or when Haiku extended the original Be concept
6. Cite specific Plan 9 concepts (`import`, `clunk`, `alarm(2)`,
   `/proc/*/ctl`, factotum) rather than vague references to "the
   Plan 9 philosophy"
7. Divergences from both traditions are tracked in serena memory:
   `pane/beapi_divergences` and `pane/plan9_divergences`

---

## Tone

Adapted from Be's voice to fit pane's context:

| Be Book | pane docs |
|---------|-----------|
| "You should know that a looper is a subclass of a handler" | "Note that Pane is NOT a looper" (state clearly, especially divergences) |
| "If your handler is limited to a certain type of messages, you can set a filter" | "If your handler only cares about certain events, add a filter" |
| "Failure to meet any of these requirements will result in your application crashing" | "Panics if the messenger has no looper channel" (Rust-idiomatic + direct) |

**Voice characteristics:**
- Direct, second person
- Consequences stated plainly
- No hedging ("may" when you mean "will")
- Practical: when describing a hook, say what developers typically do in it
- BeOS jargon only when it's the established pane term (looper, handler, messenger)

---

## What NOT to Document

- **Derive-generated trait impls**: Debug, Clone, PartialEq. The derive
  is the documentation.
- **`#[doc(hidden)]` items**: hidden for a reason.
- **Type bounds obvious from signatures**: `Send + 'static` on handler
  traits doesn't need explanation.
- **Internal plumbing**: `pub(crate)` items, `LooperMessage`.
- **Things the type system already says**: don't write "Takes a String"
  when the signature says `title: impl Into<String>`. Do write what
  the string *means*.
- **Self-evident enum variants**: `NamedColor::Red` does not need
  "the color red".

---

## Review Checklist

Before merging documentation changes:

- [ ] Every public type has `///` with at least a brief
- [ ] Every public method has `///` with at least a brief
- [ ] Core types have full treatment (overview, threading, heritage)
- [ ] `# BeOS` sections present on types/methods with Be ancestry
- [ ] `# Plan 9` sections present on types/methods with Plan 9 lineage
- [ ] Heritage sections absent on types with no precedent in either tradition
- [ ] `# BeOS` sections reference Haiku where Haiku extended the original
- [ ] `# Plan 9` sections cite specific concepts (not vague philosophy)
- [ ] `# Threading` sections present on types with threading implications
- [ ] Hook methods document trigger, default, and use case
- [ ] Message variants document trigger, dispatch target, and Be ancestor
- [ ] Crate-level `//!` contains Quick Start example
- [ ] `cargo doc` builds without warnings
