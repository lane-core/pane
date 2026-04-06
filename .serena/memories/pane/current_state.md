# Pane Project Current State (2026-04-05, end of session)

## Implementation status

Four crates, 136 unit tests + 2 doc-tests:

- **pane-proto** (12 files, 77 tests) — Protocol vocabulary, no IO. Message, Protocol, ServiceId, Flow, Handler, Handles<P>, MessageFilter, MonadicLens<S,A>, obligation handles (ReplyPort, CompletionReplyPort, CancelHandle), PeerAuth + AuthSource (transport-level peer identity).
- **pane-session** (5 files, 26 tests) — Session-typed IPC. Transport trait, MemoryTransport, bridge (two-phase handshake over par), FrameCodec (wire framing with reserved 0xFF abort, monotonic known_services bitset).
- **pane-app** (8 files, 28 tests) — Actor framework. Pane, PaneBuilder<H>, Dispatch<H> (with fire_failed tested), LooperCore<H> (catch_unwind + destruction sequence + exited guard + E-Suspend/E-React end-to-end), Messenger (stub), ServiceHandle (stub), ExitReason.
- **pane-fs** (3 files, 5 tests) — Filesystem namespace. AttrReader<S>, AttrSet<S>, AttrValue, PaneEntry<S>.

pane-notify is listed in PLAN.md as "preserved from prototype" but is not part of the redesign crate set.

Optics live in `pane-proto/src/monadic_lens.rs` (Clarke et al. Def 4.6). No pane-optic crate. No fp-library dependency. property.rs was deleted — MonadicLens supersedes Attribute.

## EAct audit status

Exhaustive audit against Fowler and Hu's "Speak Now" paper completed 2026-04-05. Two critical gaps (E-Suspend/E-React end-to-end, fire_failed/E-CancelH) closed and verified by both session-type-consultant and formal-verifier. Seven important gaps remain (multi-connection independence, double-destruction guard, CancelHandle-Dispatch integration, disconnected-Continue, AppPayload compile-fail, plus two more). See the EAct audit results in the session history.

## What's next

See PLAN.md. Phase 1 (Core) is partially complete. PeerAuth and Address implemented 2026-04-05 (both via four-agent workflow). Messenger addressing designed: `send_request` is protocol-scoped on `ServiceHandle<P>`, direct pane-to-pane communication, `Address` is lightweight copyable wire type. ConnectionSource bumped in priority (enables real routing). Next items: Framework protocols (Display, ControlMessage), then Handshake types (Hello/Welcome), then ConnectionSource + service registration.

## Doc surface

17 docs in docs/ (reduced from 24 this session). 7 deleted, 1 created (language-deliberation.md). All docs verified for consistency by formal-verifier.

## Dev workflow

See `pane/agent_workflow` for the four-agent pipeline. See `suggested_commands` for build commands, `task_completion_checklist` for post-task steps.
