# The Functoriality Principle

`Prog(Phase1 + Phase2) ≠ Prog(Phase1) + Prog(Phase2)`

The programs buildable on the full architecture are not decomposable into programs buildable on each phase independently. Phase 1 type signatures shape what developers (including us) build — a Phase 1 type that omits structure needed later produces patterns that assume that structure doesn't exist, creating an ecosystem that can't cleanly accommodate the full design.

## The Rule

Every type in Phase 1 must be the full architecture's type, populated minimally. No simplifications that create invariants later phases must break.

- `ServiceRouter` with one entry, not a bare sender
- `ServiceId { uuid, name }`, not a bare string
- `HashMap<(ConnectionId, token)>`, not `HashMap<token>`
- `PeerAuth::Kernel { uid, pid }`, not self-reported strings
- calloop ConnectionSource, not pump threads

The cost is near-zero (deterministic UUID derivation, HashMap with one entry, enum with one populated variant). The alternative is a guaranteed breaking change across every Protocol impl, Handler, and downstream application.

## The BeOS Lesson

BeOS's string-based application signatures (`application/x-vnd.Be-TRAK`) shaped an ecosystem built on `strcmp()`. When structured identity was needed (launch daemon, package management, multi-version support), everything was string-comparison all the way down. Haiku's launch daemon literally parses structure out of strings that should have been structured data from the beginning (`get_leaf` strips prefixes via `find_last_of('/')`). The type simplification in Phase 1 prevented clean evolution in Phase 2.

## The Category-Theoretic Framing

The functor from "architecture" to "programs buildable on it" is not additive over phases. Programs that only make sense with Phase 2 features (federation, structured service identity, multi-server) may require design decisions in Phase 1 that are incompatible with the Phase 1-only design space. Exposing only the simplified type shapes a design space where developers build patterns against those types — patterns that compose with the simplification but not with the full design.

## Where This Was Applied

- ServiceId { uuid, name } from day one (not &'static str → ServiceId later)
- ServiceRouter with HashMap (not bare mpsc::Sender)
- Dispatch keyed by (ConnectionId, token) (not bare token)
- PeerAuth enum (not PeerIdentity strings)
- DeclareInterest in the protocol (not implicit capability)
- Wire framing [length][service][payload] (not v1's [length][payload])
- Message enum base-protocol only, Clone-safe (not flat with panic Clone); service events via Handles<P>
- calloop ConnectionSource (not pump threads)

Documented in: `docs/workflow.md` (process rule), `docs/architecture.md` (theoretical foundation + Phase 1 structural invariants table).
