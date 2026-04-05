# Pane Project Current State (2026-04-02)

## Redesign in progress

The project is undergoing a fresh implementation based on the architecture spec (`docs/architecture.md`). The v1 prototype codebase has been struck — source files emptied for crates that require >20% rewrite. The spec is the source of truth; PLAN.md tracks execution against its four phases.

## What exists and works

- **pane-session** — session-typed channels (Chan<S,T>), transport abstraction (Unix, TCP, TLS, Memory, Proxy, Reconnecting), calloop integration. ~520 LOC, 40 tests. Orthogonal to the redesign.
- **pane-optic** — composable optic types (FieldLens, FieldAffine, FieldTraversal), composition, optic law tests. ~220 LOC. Pure crate, no protocol dependencies.
- **pane-notify** — filesystem notification abstraction (fanotify/inotify on Linux, polling stub on macOS). ~180 LOC. No protocol dependencies.
- **Nix flake** — NixOS, Darwin, sixos module definitions. Infrastructure carries forward.
- **Reference material** — Haiku Book (reference/haiku-book/), Plan 9 man pages (reference/plan9/).

## Current crates (clean slate, 2026-04-04)

Three crates, 33 tests passing:
- **pane-proto** — protocol vocabulary (Message, Protocol, ServiceId with UUID, Handles<P>, Handler, Flow, MessageFilter<M>, Property<S,A> via fp-library). No IO. Depends on fp-library, serde, uuid.
- **pane-session** — par-backed IPC channels (Chan<S,T> using par's types as phantom state, Transport trait, MemoryTransport, handshake Hello/Welcome, ProtocolAbort on Drop). Depends on par, pane-proto, serde, postcard.
- **pane-app** — EAct actor framework (Pane, PaneBuilder<H>, Dispatch<H>, Messenger, ServiceHandle<P>, ExitReason). Depends on pane-proto.

All prior crates (pane-optic, pane-notify, pane-server, pane-comp, pane-headless, pane-hello) deleted.

## What's next

See PLAN.md. No phases — the spec describes the full architecture.

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
