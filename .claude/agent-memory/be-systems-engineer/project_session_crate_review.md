---
name: pane-session crate code review findings
description: Code review of pane-session (2026-03-22) — typestate correct, crash safety holds, calloop integration needs rework before Phase 3
type: project
---

Reviewed all source and test files in pane-session. Full review at `openspec/changes/spec-tightening/review-pane-session-code.md`.

**Core verdict:** Chan<S, T> typestate is sound. Crash safety guarantee holds (Disconnected error, not panic). Unix transport framing works. Good Phase 2 deliverable.

**Critical issues:**
1. No max message size check in `recv_raw` — malformed length prefix causes unbounded allocation (unix.rs:37, calloop.rs:109)
2. Calloop `SessionSource` uses `try_clone()` + `set_nonblocking()` toggle — affects shared file description, can block compositor on partial messages

**Moderate issues:**
3. Calloop integration bypasses session types entirely (raw bytes, manual deser)
4. Framing logic duplicated between unix.rs and calloop.rs
5. `ConnectionAborted` not mapped to `Disconnected` in error.rs
6. `data.len() as u32` in send_raw silently truncates on >4GB

**Architecture note:** Current calloop path does client I/O from compositor main loop. Production architecture (per-pane threads with Chan, calloop only for connection accept + draw loop) is the right design and eliminates the calloop blocking issues. The calloop rework should happen as part of Phase 3 per-pane threading, not as a standalone fix.

**Why:** The session type primitives are the foundation for all pane protocols. Getting them right early prevents compounding errors. The calloop issues are real but scoped — they affect the compositor integration path, not the primitives themselves.

**How to apply:** When reviewing Phase 3 protocol work, verify it uses Chan on per-pane threads (not raw calloop dispatch). When the calloop rework happens, reference the blocking mode and buffering issues from this review.
