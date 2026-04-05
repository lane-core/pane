---
name: Concrete Optics Encoding Validation
description: Rigorous validation of pane's hand-rolled concrete optics design — law-preservation proofs, hierarchy analysis, required additions
type: project
---

Validated 2026-04-03 that pane's proposed concrete optics encoding (get/set fn pointers) is a legitimate, law-preserving optics library.

**Key results:**
- Mutable formulation (`fn(&mut S, A)`) is operationally equivalent to pure (`S × A → S`) under lens laws
- Composition of two lawful lenses yields a lawful composed lens (proved all three laws)
- Monomorphic (S=T, A=B) is correct for pane's use case (attribute read/write on existing handler state)
- `Option<A>` in Affine is equivalent to `Either<A, S>` in monomorphic case (by PreviewSet law, Right branch is always the input `s`)
- Clarke et al. Representation Theorem (Thm th:profrep) guarantees isomorphism with profunctor encoding for lawful optics

**Required additions for "first class" status:**
1. `over(&self, s: &mut S, f: impl FnOnce(A) -> A)` on Lens and Affine
2. Two `A` values in `assert_lens_laws` (PutPut needs distinct a1, a2)
3. `assert_affine_laws` with PreviewSet, SetPreview, SetSet, NoFocusSet tests
4. `From` conversions: Lens→Affine, Lens→Getter (hierarchy edges)

**Design decisions validated:**
- No Prism needed (Affine covers read side; `build: A → S` not needed when always operating on existing handler state)
- No Traversal/Fold needed yet (multi-focus not required in current attribute system)
- `fn` pointers for common case, `Box<dyn Fn>` for composition — dual representation is sound

**Why:** The four-agent deliberation proposed this design. This validation ensures it's not "getter/setter bags that happen to call themselves optics" but a genuine optics library grounded in the representation theorem.

**How to apply:** These findings directly shape the pane-optic crate implementation. The law test harness signatures and required additions should be implemented when building the crate.
