---
name: EAct paper analysis for pane actor model
description: Fowler et al. EventActors analysis (2026-03-29) — 6 design principles, protocol evolution recs, 6 things NOT to adopt; key insight is heterogeneous session support in looper
type: project
---

Analyzed EAct (Fowler et al., safe actor programming with multiparty session types) against pane's architecture.

**Key alignment:** Pane already satisfies KP1 (reactivity), KP2 (no explicit channels — developer never sees Chan), and has a strong KP5 (failure handling via monitor/ExitBroadcaster). Flat enum dispatch is an improvement over EAct's handler hierarchy — Rust exhaustive match gives static coverage.

**Key gaps:**
1. Active phase is untyped enums — sub-protocols (CreatePane→PaneCreated, CompletionRequest→Response, Close negotiation, future clipboard/DnD) have structure that isn't enforced
2. Single-session per looper — no multi-session support for heterogeneous sources (compositor + clipboard + audio + inter-pane)
3. Per-conversation failure callbacks missing — PaneExited says who died, not which pending request failed
4. No cascading failure / zapper-thread equivalent

**6 Design principles distilled:**
- C1: Unified event loop must support heterogeneous sessions (multi-channel select, per-channel typing)
- C2: Sub-protocols session-typed, active phase not (typed enums + sub-protocol typestates)
- C3: Failure callbacks per-conversation, not per-connection
- C4: Access points model for future service discovery
- C5: Handler installation should declare expected message types (MessageInterest)
- C6: Looper = concurrency boundary, session types = type boundary (keep orthogonal)

**Protocol evolution:** Don't session-type active phase as whole. Session-type sub-protocols via typestate on API surface (Messenger methods). Multiparty useful for reasoning, decompose to binary for implementation.

**Do NOT adopt:** flow-sensitive effects, Scribble code generation, dynamic linearity, `suspend` as primitive (pattern already exists in Handler), access points as separate abstraction (premature), become/ibecome (no blocking sends in pane).

**Why:** The most valuable EAct idea is C1 (heterogeneous session loop) — it's the genuine advance over BeOS's single-port BLooper. The most dangerous is session-typing everything — the active phase is a stream, not a conversation.

**How to apply:** When designing clipboard, DnD, observer pattern, or inter-pane messaging: structure as sub-protocols with local typestate. When evolving the looper: make it support multiple typed channels (crossbeam select or calloop multi-source). Don't change the active-phase transport.
