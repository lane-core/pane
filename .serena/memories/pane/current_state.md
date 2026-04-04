# Pane Project Current State (2026-04-02)

## Redesign in progress

The project is undergoing a fresh implementation based on the architecture spec (`docs/architecture.md`). The v1 prototype codebase has been struck — source files emptied for crates that require >20% rewrite. The spec is the source of truth; PLAN.md tracks execution against its four phases.

## What exists and works

- **pane-session** — session-typed channels (Chan<S,T>), transport abstraction (Unix, TCP, TLS, Memory, Proxy, Reconnecting), calloop integration. ~520 LOC, 40 tests. Orthogonal to the redesign.
- **pane-optic** — composable optic types (FieldLens, FieldAffine, FieldTraversal), composition, optic law tests. ~220 LOC. Pure crate, no protocol dependencies.
- **pane-notify** — filesystem notification abstraction (fanotify/inotify on Linux, polling stub on macOS). ~180 LOC. No protocol dependencies.
- **Nix flake** — NixOS, Darwin, sixos module definitions. Infrastructure carries forward.
- **Reference material** — Haiku Book (reference/haiku-book/), Plan 9 man pages (reference/plan9/).

## What was struck

Source files emptied, Cargo.toml preserved:
- **pane-app** — kit crate (Protocol, Handles<P>, Handler, DisplayHandler, PaneBuilder<H>, Message, Messenger, Dispatch<H>, ServiceHandle<P>, Flow, filter chain). Needs complete reimplementation per architecture spec.
- **pane-proto** — wire types, handshake. Needs new Protocol trait, ServiceId, ClientToServer/ServerToClient, PeerAuth.
- **pane-server** — protocol server. Needs Control protocol (wire service 0), DeclareInterest/RevokeInterest, per-connection service binding, ServiceTeardown, PaneExited broadcast, Cancel handling, max_message_size enforcement.
- **pane-comp** — compositor. Needs new protocol integration.
- **pane-headless** — headless server binary. Needs new handshake/protocol.
- **pane-hello** — example app. Direct consumer of pane-app API.

## What's next

Phase 1 — Core. See PLAN.md for detailed task breakdown. Implementation order: pane-proto → pane-session verification → pane-app → pane-server → pane-headless.

## Dev workflow

- `cargo check` / `cargo test` for surviving crates
- Specs in docs/ (architecture.md is the design spec)
- Kit-level API docs in Rust doc comments (source of truth for implemented crates)
- Style guide: `docs/kit-documentation-style.md`
- Naming guide: `docs/naming-conventions.md`
- Haiku Book at `reference/haiku-book/` — primary BeOS/Haiku API reference
- Plan 9 Programmer's Manual at `reference/plan9/` — man pages + paper sources
- serena is sole memory system. Divergence trackers: `pane/beapi_divergences`, `pane/plan9_divergences`
- Be engineer + Plan 9 engineer must be consulted before new subsystems

## Design guidance

All protocol work designed against the EAct-derived session-type principles:
- `pane/session_type_design_principles` — 6 principles (C1–C6)
- `pane/eact_analysis_gaps` — 4 structural gaps
- `pane/eact_what_not_to_adopt` — anti-patterns
- `pane/plan9_reference_insights` — Plan 9 patterns per subsystem

Key: sub-protocols use typestate handles (C2), new channels are separate typed sources (C1), failure modes use per-request Dispatch entries (C3).
