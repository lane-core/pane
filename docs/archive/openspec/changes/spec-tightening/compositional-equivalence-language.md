# Compositional Equivalence — Spec Language

Concrete language to insert into the foundations and architecture specs, codifying the invariant: composition structure between panes is preserved across all views.

---

## 1. Foundations §2 — The Pane as Universal Object

Insert after the paragraph beginning "Panes compose. Two panes viewed together form a compound structure...":

> The optics discipline (§4) applies to composition structure, not only to individual pane state. If two panes are in a composition relationship — a split, a stack, a group — that relationship is visible through every view: as spatial arrangement on screen, as directory structure under `/srv/pane/`, as sub-session nesting in the protocol, as a containment relation in the accessibility tree. The representations differ — each view expresses the relationship in its own terms — but the structural fact that the relationship exists is consistent across all of them. A relationship visible in one view and absent from another is a bug, not a feature gap.

---

## 2. Foundations §4 — Multiple Views Through Optics

Insert after the "Composition" subsection ("Optics compose. The projection from internal state to protocol messages..."):

> The lens laws extend to composition structure. GetPut and PutGet govern not only the state of individual panes but the relationships between them. Read a composition relationship through the filesystem view and write it back unchanged — the layout tree is unchanged. Create a split through the protocol and read the filesystem — the split is there. Violations under failure are temporary, subject to the same recovery semantics as individual-state violations. Intentionally lossy projections — a view that elides nesting depth, a protocol that batches structural changes — are documented, not silent. The invariant is: every view agrees on what compositions exist, even when they represent those compositions differently.

---

## 3. Architecture §2 — Pane Composition

Insert after the paragraph beginning "Panes compose spatially...":

> **Compositional equivalence.** The layout tree, pane-fs, and the pane protocol must encode composition relationships consistently. A split in the layout tree has a corresponding directory in pane-fs and emits structural events through the protocol. No composition primitive exists in one view without a representation in the others. Concretely: introducing a new composition mode (tabbed stacking, linked groups, transient overlays) requires filesystem and protocol representations before it ships. The test is automation-complete: for any composition relationship, a script must be able to discover that it exists, query its properties, and dissolve it through the standard protocol without special-case APIs.

---

## 4. Architecture §2 — pane-fs

Insert after the existing pane-fs node structure (`tag`, `body`, `attrs/`, `ctl`):

> **Composition in the filesystem.** When panes are composed, pane-fs reflects the composition structure as directory nesting. A split containing panes A and B appears as a directory under `/srv/pane/` with its own `attrs/` (encoding orientation, ratio, and split type) and child entries `A/` and `B/`. Independent panes are top-level entries; composed panes are nested under their container. The filesystem tree mirrors the layout tree's nesting — not as a consequence of the compositional equivalence invariant, but as the filesystem's native expression of it. Reparenting a pane (moving it into or out of a split) changes its position in the filesystem hierarchy. Tools that walk `/srv/pane/` see composition structure directly; they do not need to reconstruct it from per-pane geometry attributes.
