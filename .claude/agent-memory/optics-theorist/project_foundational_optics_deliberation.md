---
name: Foundational Optics Deliberation
description: Deep analysis of what-if profunctor optics were pane's foundational state access mechanism — seven dimensions evaluated, net assessment
type: project
---

Lane requested deep deliberation on 2026-04-05: what if profunctor optics were THE organizing principle for pane, not a side concern?

**Net assessment:** Viable but incremental, not revolutionary. The current architecture already uses optics at the right boundary (pane-fs). Making them "foundational" means expanding optic scope from attrs to all handler state, defining a generic state-access protocol, and using the optic registry as the single source of truth for externally-visible state. This is an extension of the existing design, not a rewrite.

**Seven dimensions analyzed:**

1. **Optic vocabulary:** Pane needs Iso, Lens, Prism, AffineTraversal, Traversal, Getter. Does NOT need Grate, Glass, Kaleidoscope. Prism matches message dispatch, but Rust's exhaustive match IS the concrete Prism encoding — making it explicit adds no value.

2. **Optics x session types:** These are orthogonal. No known categorical unification. A remote Lens decomposes into session steps (Get/Set). Session duality (Send/Recv) is parallel to optic op-duality (Lens/Prism) but they're in different categories. Independence means clean separation, but no deep unification.

3. **Optic dispatch:** Does NOT replace match-based dispatch for events. Would need runtime path resolution (HashMap lookup, introduces failure mode). Hybrid is best: optics for state access, match for events. Current ~80/20 ctl/optic boundary holds.

4. **Rust encoding:** Concrete encoding is the only viable choice. Rank-2 types impossible in Rust. Representation theorem (Clarke et al. Thm th:profrep) justifies concrete encoding as isomorphic. Runtime composition requires concrete encoding (closures capture runtime values). Even if optics were foundational, the encoding would still be concrete.

5. **Laws as invariants:** Laws hold within linearizable scope. Looper serializes local access (laws unconditional). Snapshots are immutable (laws hold within snapshot). Remote access serializes through authoritative looper. Don't weaken the laws — weaken the scope of the guarantee.

6. **Obligation handles:** Outside the optic discipline entirely, and correctly so. Linear resources need linear lenses (Clarke et al. Def 4.12), which are in a DIFFERENT category from standard lenses. Composition between linear and non-linear is asymmetric. Current architecture already correctly separates obligations from the optic path.

7. **Performance:** Negligible. Function pointer Lens = single field load. Composed optics add ~15ns per event at 1kHz. Monomorphization eliminates overhead for compile-time optics. Only pays at type-erasure boundaries.

**What changes if foundational:**
- Derive macro generates optics for ALL handler fields (not just registered attrs)
- Generic state-access protocol (optic path + operation) subsumes per-service attribute access
- Optic registry = single source of truth for externally-visible state
- Distributed state falls out of architecture (optics compose across network boundaries)

**What stays unchanged:** Match-based dispatch, obligation handles outside optics, concrete encoding, session types orthogonal, looper single-threaded.

**Why:** Determines whether optics expansion is on the roadmap as a design direction vs. a peripheral concern.

**How to apply:** If Lane decides to pursue, the path is: derive macro for handler state optics, generic state-access protocol on par channels, optic registry replacing AttrSet. If not, the current design is already sound and optics stay in their current scope.
