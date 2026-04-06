# Optics Scope Deliberation

Merged from Plan 9 systems engineer (9P-as-optic mapping) and optics theorist (seven-dimension scope analysis). 2026-04-05.

## 9P-as-optic composition (Plan 9 perspective)

The structural mapping is real, not metaphorical: walk = compose, read = get, write = set, fid = composed optic handle, mount table = optic scope.

### Key decisions

1. **Don't rebuild around optics as primitive.** Keep optics as correctness criterion (the meaning), keep the protocol as mechanism (the wire format). Plan 9's power came from 9P's simplicity.
2. **Per-connection optic laws hold.** GetPut/PutGet/PutPut guaranteed within a single connection (FIFO). Cross-connection requires optimistic versioning (qid.vers analogue).
3. **Walk steps are affine, not lens.** Each walk step can fail (Rerror), so the composition is AffineTraversal. The fid is a witness that the walk succeeded.
4. **Event streams and ctl files are NOT optic operations.** ~80% of pane-fs fits the optic model; acknowledge the ~20% that doesn't.
5. **Failure: keep Plan 9's model.** ServiceHandle type stays the same when connection fails; operations return Result.
6. **Three-tier model resolves bytes-vs-typed tension.** Filesystem tier = byte-stream optic (Lens composed with text serialization Iso). Protocol tier = typed optic.
7. **Explicit suspension over transparent.** Optics invalidated at suspension, re-composed at resumption.

## Seven-dimension scope analysis (optics theorist perspective)

**Net assessment:** Viable but incremental, not revolutionary. Current architecture already uses optics at the right boundary.

1. **Optic vocabulary needed:** Iso, Lens, Prism, AffineTraversal, Traversal, Getter. NOT needed: Grate, Glass, Kaleidoscope. Prism matches message dispatch, but Rust's exhaustive match IS the concrete Prism — making it explicit adds no value.
2. **Optics x session types:** Orthogonal. No known categorical unification. Independence means clean separation but no deep synthesis.
3. **Optic dispatch does NOT replace match-based dispatch** for events. Current ~80/20 ctl/optic boundary holds.
4. **Rust encoding:** Concrete encoding is the only viable choice. Rank-2 types impossible. Representation theorem (Clarke et al. Thm th:profrep) justifies concrete as isomorphic.
5. **Laws hold within linearizable scope.** Looper serializes local access (unconditional). Snapshots are immutable (within snapshot). Remote access serializes through authoritative looper.
6. **Obligation handles outside optic discipline entirely** and correctly so. Linear resources need linear lenses (different category). Current architecture already correctly separates them.
7. **Performance negligible.** Function pointer Lens = single field load. ~15ns per event at 1kHz. Monomorphization eliminates overhead for compile-time optics.

## What changes if foundational

- Derive macro generates optics for ALL handler fields (not just registered attrs)
- Generic state-access protocol subsumes per-service attribute access
- Optic registry = single source of truth for externally-visible state
- Distributed state falls out of architecture

## What stays unchanged regardless

Match-based dispatch, obligation handles outside optics, concrete encoding, session types orthogonal, looper single-threaded.

**How to apply:** Optics as semantic model and test criterion, protocol as mechanism. Don't merge them. The simplicity is load-bearing. If expanding optic scope: derive macro → generic state-access protocol → optic registry.