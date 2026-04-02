---
name: Handler debate final assessment (2026-03-31)
description: Three-round debate conclusion — commit register_channel + EventKind + commit()->Result now, defer Handler split until observer+DnD ship, five invariants
type: project
---

Final assessment after three-round debate between session-type consultant, Be engineer, and Plan 9 engineer.

**Verdict**: Be engineer's proposal is conditionally sound. Commit mechanism layer (Phase 3) now, defer API surface layer (Phase 4 trait split) until 3+ services establish the pattern.

**Commit now:**
1. `register_channel<E>(handle, channel, convert: Fn(E) -> Message)` — Phase 3 mechanism
2. `EventKind` discriminant enum + remove `Message::Clone` — eliminate 4 panic branches
3. `commit() -> Result<(), ClipboardError>` — race window safety
4. Preserve five typestate handles (ReplyPort, CompletionReplyPort, ClipboardWriteLock, PaneCreateFuture, TimerToken)

**Defer:**
5. Handler trait splitting — until observer + DnD arrive (2 more protocols with phase structure)
6. ClipboardCallbacks struct — defer alongside trait split, must route through unified batch

**Why I withdrew the immediate Pane-as-trait push:**
EAct E-React indexes handlers by session endpoint in sigma, but pane's sequential single-threaded dispatch already satisfies this operationally via `dispatch_to_handler` match. Type-level indexing (trait per protocol) would add compile-time enforcement of something already operationally enforced. Cost > benefit without 3 concrete instances.

**Five minimum invariants to not regress:**
1. Typestate handles: #[must_use] + Drop failure-terminal + move-only + single success method
2. No handler method holds two reply obligations simultaneously
3. Filter chain: discriminant-based matching for linear variants (EventKind proposal)
4. Looper thread detection: is_looper_thread() prevents send_and_wait deadlock
5. register_channel convert function: total, non-blocking, Fn(E) -> Message

**Key citations grounding the assessment:**
- EAct Progress Theorem 3.10, Global Progress Corollary 3.14, E-React rule (§3.2 Fig. 4), KP3
- DLfActRiS POPL 2024 §3.2 channel fulfillment, §4 Definition 4.1 strong acyclicity
- TLL+C §2 Ch(T)/Hc(T) channel type rules
- Ferrite ECOOP 2022 §4.2 Theorem 4.3 (protocol fidelity) — what pane does NOT have but doesn't need yet

**How to apply:** This is the settled architecture direction. Phase 3 implements items 1-3. Phase 4 revisits items 5-6 after observer and DnD ship. The five invariants are non-negotiable constraints on all future work.
