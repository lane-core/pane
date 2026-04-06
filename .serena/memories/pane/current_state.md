# Pane Project Current State (2026-04-05)

## Implementation status

Four crates, 93 tests:

- **pane-proto** (10 files, 43 tests) — Protocol vocabulary, no IO. Message, Protocol, ServiceId, Flow, Handler, Handles<P>, MessageFilter, MonadicLens<S,A>, obligation handles (ReplyPort, CompletionReplyPort, CancelHandle).
- **pane-session** (5 files, 25 tests) — Session-typed IPC. Transport trait, MemoryTransport, bridge (two-phase handshake over par), FrameCodec (wire framing with reserved 0xFF abort).
- **pane-app** (8 files, 20 tests) — Actor framework. Pane, PaneBuilder<H>, Dispatch<H>, LooperCore<H> (catch_unwind + destruction sequence + exited guard), Messenger (stub), ServiceHandle (stub), ExitReason.
- **pane-fs** (3 files, 5 tests) — Filesystem namespace. AttrReader<S>, AttrSet<S>, AttrValue, PaneEntry<S>.

pane-notify is listed in PLAN.md as "preserved from prototype" but is not part of the redesign crate set.

Optics live in `pane-proto/src/monadic_lens.rs` (Clarke et al. Def 4.6). No pane-optic crate. No fp-library dependency.

## What's next

See PLAN.md. Phase 1 (Core) is partially complete. Remaining: Display protocol, PeerAuth, handshake types, DeclareInterest, Cancel, ProtocolHandler derive macro, Messenger full impl, ConnectionSource, service registration, looper (calloop), AppPayload, server crate, headless binary.

## Dev workflow

See `suggested_commands` for build commands, `task_completion_checklist` for post-task steps.

- Specs in `docs/` — `architecture.md` is the design spec
- Kit API docs in Rust doc comments (source of truth for implemented crates)
- Style: `docs/kit-documentation-style.md`
- Naming: `docs/naming-conventions.md`
- Haiku Book at `reference/haiku-book/`
- Plan 9 material at `reference/plan9/`
- Divergence trackers: `pane/beapi_divergences`, `pane/plan9_divergences`
- Consult Be engineer + Plan 9 engineer before new subsystems
- Four-agent workflow: see `pane/agent_workflow`
