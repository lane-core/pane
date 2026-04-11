---
type: policy
status: current
supersedes: [auto-memory/feedback_per_pane_threading]
created: 2026-04-06
last_updated: 2026-04-10
importance: high
keywords: [per_pane_threading, isolation, blocking, backpressure, I2, BeOS_per_window]
agents: [pane-architect, session-type-consultant, formal-verifier]
---

# pane IS per-pane threading — don't misanalyze the isolation boundary

**Rule:** Each pane has its own looper thread. Cross-pane isolation is structural. Intra-pane blocking is legitimate backpressure.

The session-type consultant initially recommended `try_send` (non-blocking) for the write channel, citing I2 ("no blocking in handlers"). Lane caught the error: pane IS per-pane threading, same as Be's per-window model. If pane A blocks on send, only pane A stalls — pane B is unaffected. Blocking within a pane is legitimate backpressure, not a violation.

**Why:** I2 prevents cross-pane interference, which per-pane threading already provides. The invariant was overconstrained — it conflated cross-pane interference (real problem) with intra-pane backpressure (not a problem). The session-type consultant revised their position.

**How to apply:** When analyzing blocking, deadlock, or backpressure in pane, always check: does this affect only the current pane's looper thread, or does it cross pane boundaries? Same-pane blocking is backpressure. Cross-pane blocking is a bug. Be's `write_port` with `B_RELATIVE_TIMEOUT` is the reference model.
