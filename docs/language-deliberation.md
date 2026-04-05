# Language Split Deliberation

Extracted from a structured deliberation involving four specialist
agents (optics theorist, session type consultant, Be systems
engineer, Plan 9 systems engineer). This section evaluated
whether pane's core should be implemented in a functional
language rather than Rust.

---

## Verdict

Stay with Rust. The migration cost exceeds the gains for pane's
current protocol surface.

## Strongest argument for OCaml

Native algebraic effects eliminate bridge threads and give direct-style
`send_request`. OCaml's mutable record fields preserve the `&mut self`
feel of Handler methods. Jane Street's `accessor` library encodes the optic
subtyping lattice as row-polymorphic variant types. OCaml 5 domains provide
multicore. CBV evaluation makes obligation handle lifetimes predictable.

## Strongest argument for Haskell

`LinearTypes` extension (`%1 ->`) closes the affine gap. Rich optics
ecosystem (lens, optics, generic-lens). Typeclasses express protocol
interfaces more naturally than Rust traits.

## Why neither (now)

1. **par is the most production-ready session type implementation in any
   language.** Neither OCaml nor Haskell has a mature equivalent. Trading a
   working system for one built from scratch is wrong.

2. **Postcard must be replaced before splitting.** Postcard is Rust-native,
   serde-coupled, no cross-language implementations. The wire format must
   be language-agnostic first. This is valuable regardless of the language
   question.

3. **The type-level protocol work that would benefit from a functional
   language is already done** (par). The developer-facing layer (pane-app)
   is inherently imperative — an event loop dispatching to mutable handlers.

4. **One toolchain is worth more than the expressiveness gap** for a
   pre-1.0 project where the protocol isn't stable.

## What to do now (regardless of language choice)

1. **Replace postcard on the wire.** Hand-specified binary for control
   messages, CBOR for service payloads. This unblocks the option to split
   later.

2. **Write a byte-level protocol specification.** Not Rust types — a wire
   format document. Every message, every field, every encoding.

3. **Build language-independent conformance tests.** Wire traces (hex
   dumps) that any implementation can verify against.

4. **Evaluate OCaml after Phase 1 ships.** The split should be motivated
   by real pain, not anticipated elegance.

## Lane's CBV hunch

Partially confirmed. OCaml's CBV eliminates strictness annotation burden
(no `NFData` on every sent type), makes obligation handle construction
predictable (allocated immediately, Drop fires predictably), and matches
pane's dispatch model (each step evaluated eagerly). Gay/Vasconcelos JFP
2010 S5 proves session fidelity holds under both evaluation strategies, so
it's ergonomic, not formal.

## Inferno/Limbo as precedent

The closest historical parallel. Limbo was strict, GC'd, with channels
and modules. It implemented 9P servers successfully. OCaml is strictly
more capable. The Inferno lesson: the split works when the protocol
boundary is clean, but the ecosystem cost was real (nobody used Limbo
outside Inferno). Pane avoids this by keeping Rust as the ecosystem-facing
language.
