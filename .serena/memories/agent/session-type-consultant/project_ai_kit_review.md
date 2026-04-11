---
name: AI Kit session-type review (2026-04-03)
description: Review of docs/ai-kit.md — conditionally sound, 7 issues (1 moderate, 6 minor), agents are protocol participants not special category
type: project
---

AI Kit spec review from session-type perspective. 2026-04-03.

**Verdict:** Conditionally sound. Agents are ordinary Handler/Handles<P> participants. No new protocol, no new session-type surface. All architecture spec invariants (I1-I13, S1-S6) apply identically.

**Moderate issue:**
- §6 /pane/<n>/event blocking-read MUST NOT occur on looper thread (I2 violation, EAct Progress). Spec should state this explicitly.

**Minor issues:**
- §3 says "typed synchronous" — should say "typed protocol" (send_request is async with callback)
- §1 crash safety implicitly depends on I1 (panic=unwind) but doesn't state it
- §1 restarted agent gets new pane Id; monitoring by Id vs by-sig index differs in resilience
- §6 event+ctl coordination lacks protocol fidelity that Handles<P> provides — should acknowledge
- §3 ctl error propagation to write(2) caller unspecified
- §9 should mention request_received as untyped escape hatch, recommend Handles<P>

**Confirmed correct:**
- Crash safety sequence matches architecture spec §Termination exactly
- Cross-agent typed/untyped boundary is clear and correctly documented
- Sub-agent VM gap is deliberate and correct — avoids cross-VM deadlock topology problems (DLfActRiS §4 Def 4.1)
- ctl vs attrs distinction is sound — ctl is imperative/untyped, attrs is optic-governed
- Event file is outside typed protocol (filesystem projection, not session channel) — correct boundary

**Why:** Establishes session-type soundness of agent participation model before Phase 1 implementation.
**How to apply:** Reference when implementing agent pane support. The /pane/<n>/event looper-thread constraint is the one item that needs design enforcement (worker thread or pre-looper main loop only).
