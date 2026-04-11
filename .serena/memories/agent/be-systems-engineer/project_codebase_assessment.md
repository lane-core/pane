---
name: Codebase assessment (2026-03-21)
description: Current state of pane Rust codebase — two crates, significant stale code from pre-spec architecture, refactor proposal written
type: project
---

Assessed all source files in pane's Rust codebase (2026-03-21). Two crates exist:

**pane-proto:** Wire types + runtime protocol state machine. ~50% stale. Stale artifacts: ProtocolState runtime state machine (replaced by session types), Value/Compute polarity traits (sequent calculus framing dropped), PaneMessage<T> attrs bag (replaced by typed enums), ServerVerb + TypedView + route/roster modules (central router eliminated), WidgetNode/WidgetEvent (server-rendered widgets contradict client-side rendering spec), optics dependency (unused). Salvageable: wire.rs framing, event.rs input types, tag.rs, color.rs, PaneId.

**pane-comp:** Single-pane demo in winit window. No calloop event loop (uses thread::sleep), no Wayland protocol, no client sockets, no layout tree. Only glyph_atlas.rs approach is reusable. Everything else is Phase 4 rewrite.

**Key architectural conflict:** CellGrid/Widget PaneKind variants assume compositor-rendered body content. Spec commits to client-side rendering (Wayland model). Cell types move from wire protocol to pane-ui client-side rendering.

**Refactor proposal:** Written to openspec/changes/spec-tightening/proposal-codebase-refactor.md. Phase 1: delete stale code (half-day). Phase 2: create pane-session crate (custom session types). Phase 3+: new crates built on session-typed foundation.

**Why:** Session types replace both the runtime state machine AND the TypedView pattern. The entire server/ module and state.rs exist to solve problems that compile-time protocol verification makes unnecessary.

**How to apply:** Phase 1 cleanup is prerequisite for Phase 2 session type work. Don't start pane-session until pane-proto is clean — otherwise the new crate will be built against types that are about to change.
