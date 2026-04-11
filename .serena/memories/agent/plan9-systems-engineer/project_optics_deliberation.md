---
name: 9P-as-optic-composition deliberation
description: Deep assessment of mapping 9P operations to profunctor optics for pane's distributed architecture — structural correspondence, limits, and recommendation (2026-04-05)
type: project
---

Lane proposed making profunctor optics THE mechanism for distributed state access, with 9P walks as optic compositions. Ten-question deliberation produced these findings:

## Core finding: the structural mapping is real, not metaphorical

walk = compose, read = get, write = set, fid = composed optic handle, mount table = optic scope. These are genuine structural isomorphisms. A 9P file server IS an optic server in a formal sense.

## Key decisions / recommendations

1. **Don't rebuild around optics as primitive.** Keep optics as correctness criterion (the meaning), keep the protocol as mechanism (the wire format). Plan 9's power came from 9P's simplicity, not from encoding mathematical structure on the wire.

2. **Per-connection optic laws hold.** GetPut/PutGet/PutPut guaranteed within a single connection (FIFO). Cross-connection requires optimistic versioning (qid.vers analogue) if conflicts matter. This matches Plan 9's consistency model.

3. **Walk steps are affine, not lens.** Each walk step can fail (Rerror), so the composition is AffineTraversal, not Lens. The fid is a witness that the walk succeeded.

4. **Event streams and ctl files are NOT optic operations.** Events are morphisms (state transitions), not objects (state values). Ctl is an effect channel. ~80% of pane-fs fits the optic model; acknowledge the ~20% that doesn't.

5. **Service map IS an optic scope (mount table).** Making this explicit improves documentation but adding typed optic resolution (capability-to-provider matching) is premature — defer to Phase 3+.

6. **Failure: keep Option C (Plan 9's model).** ServiceHandle type stays the same when connection fails; operations return Result. Don't try to encode connection health in the optic type.

7. **Authentication capabilities map cleanly** (r = Getter, w = Setter, rw = Lens) but enforce at operation level for Phase 1-2, not via capability-restricted optic composition.

8. **Three-tier model resolves bytes-vs-typed tension.** Filesystem tier = byte-stream optic (Lens composed with text serialization Iso). Protocol tier = typed optic. The composition preserves laws.

9. **Explicit suspension over aan transparency.** Optics invalidated at suspension, re-composed at resumption. Honest about failure.

**Why:** Lane exploring whether optics should be promoted from design principle to foundational mechanism in a potential radical redesign.

**How to apply:** Reference when optics-first architecture questions arise. The recommendation is: optics as semantic model and test criterion, protocol as mechanism. Don't merge them. The simplicity is load-bearing.
