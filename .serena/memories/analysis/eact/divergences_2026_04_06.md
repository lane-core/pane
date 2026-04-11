---
type: analysis
status: needs_verification
sources: [.serena/memories/pane/eact_divergence_audit]
verified_against: [PLAN.md as of 2026-04-06; partially superseded by post-2026-04-06 work]
created: 2026-04-06
last_updated: 2026-04-11
importance: high
keywords: [eact, divergence_audit, fowler, hu, e_self, e_cancelmsg, e_spawn, ibecome, e_monitor, four_agent_roundtable, I9_regression]
related: [analysis/eact/audit_2026_04_05, reference/papers/eact, architecture/looper, decision/wire_framing]
agents: [session-type-consultant, formal-verifier, plan9-systems-engineer, be-systems-engineer, pane-architect]
---

# EAct Divergence Audit (2026-04-06, session 3)

**Status: needs_verification.** The "15 of 19 invariants
verified; 4 not yet applicable" line below is stale — all 19
are now verified or detection-enforced (see `status`). The
divergence verdicts (E-CancelMsg, E-Spawn, ibecome, E-Monitor)
remain valid. Phase 6 will fold this into `analysis/eact/_hub`.

---

Four-agent roundtable unanimously vetted pane's intentional
divergences from EAct (Fowler / Hu). All three safety theorems
(Preservation, Progress, Global Progress) hold. **15 of 19
invariants verified; 4 not yet applicable** (I2 / I3
conventions requiring timeout watchdog, I8 `send_and_wait`
unimplemented, S3 batch processing requires calloop) — **all
four have since been resolved** (see `status` and
`architecture/looper`).

## E-Self: no formal rule in EAct

There is no E-Self rule. Self-messaging is mentioned once
(line 894) as informal KP4 description. Cross-session
interaction is achieved through shared handler state
(`&mut self`), which pane implements. Safely deferrable —
implement as `Messenger::post_to_self()` for convenience when
needed.

## Divergence verdicts (4–0 unanimous)

### SAFE: E-CancelMsg (channel closure instead of explicit drain)

- pane's `fail_connection` + obligation handle Drop is more
  thorough than BeOS drain or Plan 9 cascade
- Structural precondition: `Message: Clone + Serialize`
  excludes linear resources from queues
- **INVARIANT:** If `Message` bounds are ever relaxed to allow
  capabilities, draining must be reconsidered

### SAFE: E-Spawn (OS-level panes, not handler-spawnable)

- Plan 9 kernel handlers don't spawn; exportfs spawns = bridge
  threads
- BeOS unsupervised handler-spawned loopers caused orphan bugs
- EAct independence theorem (4.5): spawn order doesn't matter

### SAFE: ibecome (not adopted)

- Paper itself says system without ibecome enjoys global
  progress (§5)
- Non-blocking sends eliminate the motivating problem
- Per-pane threading + SyncSender is strictly better than
  coroutine yield

### RESOLVED: E-Monitor (pane death notification) — was RISKY, now implemented

- Watch / Unwatch / PaneExited ControlMessage variants on
  ProtocolServer
- Fire-and-forget registration (subscription, not obligation)
- One-shot delivery (EAct E-InvokeM semantics)
- Server cleans up on watcher disconnect; watch-when-already-dead
  sends immediately
- `Handler::pane_exited` callback on Handler trait
- ~~`Messenger::watch` / `unwatch` stubs (need write_tx on Messenger)~~
  — still pending wiring

## I9 regression found and fixed

Formal-verifier final sweep found Reply / Failed dispatch
branches lacked `catch_unwind` — panicking on_reply / on_failed
callbacks would skip destruction sequence. Fixed in commit
`6e0130b`. All 15 verifiable invariants now pass.

(Post-2026-04-06: all 19 invariants pass.
See `architecture/looper` and `status` for the current state.)

## Phase 6 disposition

This audit becomes a spoke in `analysis/eact/_hub` when Phase 6
builds the eact cluster. The four divergence verdicts are
load-bearing; the invariant count needs refreshing at hub-build
time.
