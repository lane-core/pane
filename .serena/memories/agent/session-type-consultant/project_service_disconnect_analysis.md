---
name: Service disconnection notification analysis
description: Four-question analysis of clipboard service disconnect surfacing — proactive required, generic variant, commit() must return Result, not opt-in
type: project
---

Service disconnection notification analyzed 2026-03-31.

**Verdict**: Option 1 (proactive notification) is required. Option 3 (fail-at-use-site) is insufficient.

**Key decisions**:

1. **Proactive notification required** (C3 violation otherwise). Drop-revert compensates handler-side affine gap, NOT service-side failure. Handler holding ClipboardWriteLock after service death gets silent commit failure — must be told.

2. **Generic `ServiceDisconnected { service: &'static str }` variant** — one variant covers all services. The `&'static str` tag is the channel name from `register_channel`. O(1) growth vs O(n) for per-service variants. Loss of compile-time exhaustiveness acceptable because service disconnect is abnormal event with uniform default (continue).

3. **`commit()` must return `Result<(), ClipboardError>`** — synchronous at-call-site failure detection. Notification is asynchronous (next batch). Both needed: `commit() -> Result` for the race window, `ServiceDisconnected` for state cleanup/UX. Neither alone is sufficient.

4. **NOT opt-in** — clipboard service is infrastructure, not a monitored peer. Handler that has a clipboard channel gets the notification automatically via `register_channel`'s `on_disconnect`. Making it opt-in creates ghost-state anti-pattern (runtime correlation between "registered for health" and "safe to use clipboard").

**Two-mechanism recommendation**:
- `commit() -> Result` — synchronous, at-call-site
- `ServiceDisconnected` via `on_disconnect` — asynchronous, handler-level

**Why:** Finalizing Phase 3 disconnect surfacing before implementation.
**How to apply:** These four verdicts complete the Phase 3 design alongside the channel topology decisions.
