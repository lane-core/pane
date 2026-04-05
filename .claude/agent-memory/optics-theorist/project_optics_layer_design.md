---
name: Optics Layer Design Analysis
description: Design decisions and analysis for pane's optic-backed attribute system — hierarchy, type erasure, composition, snapshot model
type: project
---

Comprehensive optics layer design analysis completed 2026-04-03.

**Key design decisions presented (awaiting Lane's selection):**

1. **Attribute hierarchy:** Option A (separate structs: Attribute wrapping Lens, OptionalAttribute wrapping AffineTraversal, ComputedAttribute wrapping Getter) recommended over enum or trait-based approaches. Preserves compile-time distinction between total and optional access.

2. **Type erasure boundary:** AttrReader (current, read-only) needs extension with AttrWriter for write path. AttrValue enum (String/Bool/Int/Float/Bytes/Rect from optics-design-brief.md) proposed as intermediate format rather than raw Display/FromStr.

3. **Composition with type erasure:** Three approaches analyzed. Approach A (flatten at declaration, compose in typed world) for simple cases, Approach B (hierarchical AttrSet with sub-state extraction) for nested attributes. Approach C (DynOptic registry, compose lazily) deferred as premature.

4. **Snapshot model:** Current clone-and-snapshot is Store comonad pattern. Recommended immediate snapshot push after writes to narrow staleness.

5. **Law testing harness:** assert_lens_laws and assert_affine_laws functions proposed for pane-proto::testing.

**Why:** The optics layer is foundational — every subsystem (clipboard, observer, DnD) crosses the type-erasure boundary between typed handler state and the filesystem/scripting interface.

**How to apply:** These decisions shape pane-optic crate structure, AttrSet API, and the write path through the looper.
