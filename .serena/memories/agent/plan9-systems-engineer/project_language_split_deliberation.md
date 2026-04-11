---
name: Language split deliberation
description: Core in OCaml/Haskell with Rust clients — postcard blocks this, CBOR or hand-specified format needed, split after Phase 1 not before (2026-04-05)
type: project
---

Lane explored implementing pane's core subsystems in Haskell or OCaml, keeping Rust for ecosystem components (smithay, FUSE, TLS). Wire protocol would be the only interface.

## Key findings

1. **Postcard is the blocker.** It has no language-independent spec worth the name; enum encoding depends on Rust source variant ordering; no schema; positional struct serialization. Must be replaced on the wire for any cross-language work. Fine for Rust-internal storage.

2. **Wire format recommendation:** Hand-specified binary for control messages (15 message types, define each explicitly like 9P's intro(5)), CBOR (RFC 8949) for service payloads where extensibility matters.

3. **Inferno validates the approach.** Limbo (GC, strict, algebraic, channels) served 9P successfully. Performance ceiling existed but was in the VM, not the language model. OCaml does not have this problem.

4. **OCaml favored over Haskell.** Strict evaluation = predictable latency. Module system fits pane's service model better than typeclasses. Inferno's Limbo was strict for the same reasons.

5. **Namespace serving: core should serve it directly (Option A).** Plan 9 principle: process that owns state serves the namespace. pane-fs moves with the core if core changes language.

6. **Recommendation: don't split yet, design as if you will.** Replace postcard on wire now. Write byte-level protocol spec. Build language-independent conformance tests (hex trace capture). Evaluate OCaml after Phase 1 ships when protocol is exercised and pain points are concrete.

**Why:** Lane is considering whether pane's algebraic abstractions (session types, optics, protocol dispatch) are better expressed in OCaml/Haskell than Rust. Plan 9's philosophy supports language-agnostic protocols but warns against premature splits.

**How to apply:** When wire format decisions arise, push for language-agnostic encoding. When postcard usage is discussed, flag that it blocks cross-language implementation. Reference Inferno precedent when the split question resurfaces.
