---
name: Scripting optics design exploration
description: Phase 6 design exploration (2026-03-30) — concrete optics + DynOptic, flat request-response session, ResolveSpecifier as trait-object chain, affine as default optic kind
type: project
---

Completed design exploration for scripting protocol combining optics with session types. Document at `docs/scripting-optics-design.md`.

**Key decisions:**
1. Concrete monomorphic optic structs (Lens/Prism/Affine/Traversal) within handlers, `DynOptic` trait object at protocol boundary — profunctor encoding fights Rust's type system for no gain
2. Scripting session type is simple request-response (`Send<ScriptQuery, Recv<ScriptResponse, End>>`), NOT recursive — BeOS's ResolveSpecifier was internal to a single app, the chain walk is ordinary method dispatch
3. Dynamic composition via trait-object chain matching BeOS's pattern exactly — `ScriptableHandler` trait with `resolve_specifier` + `supported_properties`
4. Multi-view consistency via single source of truth with demand-driven projection — standard Model-View, optics formalize the projections
5. Affine optics (not lenses) as default because targets may not exist
6. `#[derive(Scriptable)]` macro generates optic registrations from `#[scriptable]` field attributes

**Why:** The profunctor representation theorem (Clarke et al. Theorem 4.4) shows optics compose as functions, but Rust lacks the rank-2 polymorphism to express this. The concrete encoding works because the two composition contexts (within-handler: static/monomorphic, across-handler: dynamic/erased) map to different Rust mechanisms. The session type is simple because the protocol boundary is at the application edge.

**How to apply:** When implementing Phase 6, start with DynOptic trait + hand-written optics for 2-3 properties, then ScriptableHandler + resolution loop, then derive macro. The session type infrastructure already exists in pane-session and needs no changes for scripting v1.

**Papers referenced:**
- Clarke et al. "Profunctor optics, a categorical update" — Def 2.1 (optic as coend), Theorem 4.4 (profunctor representation), Prop 2.3 (optics form a category)
- "Don't Fear the Profunctor Optics" — composition as function composition, concrete-to-profunctor translation
- Fu/Xi/Das TLL+C — protocol/channel separation, ghost state discipline, linear vs affine gap analysis
