---
name: Phase 3 channel topology split analysis
description: Six-question analysis of C1 multi-source looper Phase 3 — channel registration, ClipboardEvent extraction, Handler staging, affine gaps
type: project
---

Phase 3 channel topology split analyzed 2026-03-31.

**Verdict**: Conditionally sound. The design `register_channel<E>(channel, convert)` is minimal and correct.

**Key decisions**:
1. Extract `ClipboardEvent` enum (3 variants), convert to `Message` at looper boundary (Option A). Defer Handler sub-trait split to Phase 4.
2. Generic registration: `register_channel<E: Send + 'static>(handle, channel, convert: Fn(E) -> Message)`. Events enter unified batch. No per-channel dispatch path.
3. Channel lifecycle is NOT a session — degenerate mu X. Recv<E, X>. EventLoop owns the source.
4. Clipboard channel has no affine gap. WriteLock gap is compensated. BUT: commit-after-service-disconnect silently loses data. `commit()` should return `Result`.
5. Convert-to-Message is acceptable for Phase 3. Ergonomic loss, not safety loss. Phase 4 adds ClipboardHandler sub-trait.
6. One consumer is sufficient — mechanism has no domain-specific content to distort. Add channel name (diagnostics) and disconnect callback now.

**Two additions for forward-compatibility**:
- Channel name (`&'static str`) for protocol tracing
- Per-channel disconnect callback (`on_disconnect: FnOnce() -> Message`)

**Why:** Phase 3 is the first multi-source step in C1 evolution. Getting the generic mechanism right now avoids retrofit when observer/DnD channels arrive.

**How to apply:** These six verdicts are the design constraints for Phase 3 implementation. The `register_channel` signature and the Option A conversion pattern are load-bearing.
