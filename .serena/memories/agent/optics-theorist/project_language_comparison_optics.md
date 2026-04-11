---
name: Language Comparison for Optics Expressiveness
description: Deep comparison of Haskell/OCaml/Rust for pane's optic layer — ecosystem, CBV/CBN, rank-2, FFI boundary, honest assessment
type: project
---

Analyzed 2026-04-05. Lane asked for optics-angle deliberation on implementing pane core in Haskell or OCaml.

**Key findings:**

1. **OCaml optics ecosystem:** `accessor` (Jane Street) with `ppx_accessor` derive. Van Laarhoven encoding. Row polymorphism on variant index types gives optic subtyping — arguably cleaner than Haskell's typeclass approach. No profunctor encoding in production (module-level rank-2 too verbose).

2. **CBV vs CBN:** Laziness helps optics by deferring unused computation (e.g., setter part when only viewing). For pane's access patterns (shallow composition, small state, by-reference reads from snapshots), the advantage is academic. Performance-relevant boundary is snapshot clone cost, identical across languages.

3. **What Rust genuinely can't do:** Dynamic optic construction that retains type structure at runtime. Matters if FUSE server wants to derive permissions from optic types dynamically. Currently worked around by tracking permissions separately.

4. **FFI boundary:** Optics don't need to cross the language boundary. Current AttrReader is Box<dyn Fn> — already concrete. Cross-language: serialize the result, send over protocol. Optics stay in core language.

5. **Honest pick for optic expressiveness:** Haskell > OCaml > Rust. But the gap is narrow for pane's actual use cases. A Rust derive macro closes most of the derivation ergonomics gap. The strongest language-boundary argument is session types, not optics.

**Why:** Thought experiment about whether language expressiveness justifies the complexity of a multi-language architecture.

**How to apply:** If this question comes up again, the answer is: optics alone don't justify the language switch. The gap is real but bounded, and pane's architecture already works around Rust's limitations.
