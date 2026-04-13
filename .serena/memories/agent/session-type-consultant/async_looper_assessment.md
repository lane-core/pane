---
type: analysis
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: critical
keywords: [async, sync, calloop, smol, LocalExecutor, EAct, par, Rumpsteak, deadlock, I2, N1, batch_ordering, KP4, impedance_mismatch]
sources:
  - "[FH] §3.2 (E-Send, E-React, E-Suspend), §3.3 (Lemma 1 Independence, Defn 7 Thread-Terminating), §6 (Java NIO implementation)"
  - "[JHK24] §1 Theorem 1.2 (linearity + acyclicity => global progress)"
  - "[Rumpsteak] Cutner/Yoshida/Vassor — Sink/Stream async API, Theorem 3.3 Soundness"
  - "[AGP] Scalas/Barbanera/Yoshida — asynchronous realizability"
  - "par 0.3.10 exchange.rs — Recv is async, Send is non-blocking"
  - "research/async_session_bridge — impedance mismatch analysis"
  - "agent/session-type-consultant/rumpsteak_smol_translation — Path A/B/C analysis"
  - "decision/pane_session_mpst_foundation — N1-N4 invariants, typestate gap"
verified_against:
  - "eventactors-extended.tex:1057-1061 (yield points), 3068-3070 (agnostic to termination method), 3082-3086 (independence via async comms), 4093-4094 (Java NIO runtime), 4316-4317 (yields only on receives)"
  - "par-0.3.10/src/exchange.rs (Send::send is non-blocking, Recv::recv is async)"
  - "crates/pane-app/src/looper.rs:481-521 (main loop), 533-732 (dispatch_batch)"
related: [dependency/par, decision/pane_session_mpst_foundation, research/async_session_bridge, agent/session-type-consultant/rumpsteak_smol_translation, architecture/looper, policy/feedback_per_pane_threading]
agents: [session-type-consultant]
---

# Assessment: Async is the theoretically correct model for pane's actor loop

## Verdict

Async is a strict upgrade on every session-type-relevant axis.
Conditionally sound: requires single-threaded LocalExecutor and
batch ordering preserved as sequential .await chain in one async fn.

## Key findings

1. **EAct is agnostic to sync/async.** E-Send is non-blocking,
   E-Suspend is "yield to event loop," E-React is "event loop
   invokes handler." None require synchronous execution. EAct's
   own implementation uses Java NIO (§6, l.4093). Yield points
   occur only on receives (l.4316), which maps to .await on par
   Recv.

2. **Six-phase batch ordering survives.** dispatch_batch becomes
   an async fn with sequential .awaits per phase. LocalExecutor
   cannot interleave within the batch — no concurrent futures
   exist. S3 invariant preserved by construction.

3. **par impedance mismatch eliminated.** par's Recv is a future.
   Currently bridged via noop_waker/block_on. With async looper,
   par endpoints .await directly. No bridge thread needed for
   handshake; DequeueStream workaround eliminated.

4. **Rumpsteak runtime integration enabled.** Currently limited
   to Path A (verification only) because Rumpsteak requires
   async. Async looper enables Path B/C — generated CFSM state
   types with verified deadlock freedom driving the active phase.
   Closes the typestate gap (handshake typed, active phase untyped).

5. **I2 enforcement unchanged.** Blocking the executor thread is
   the same violation as blocking the sync loop. Watchdog detects
   both identically. No new threat class.

6. **N1 unchanged.** try_send + fallback is the mechanism in both
   models. Async send().await on full channel blocks the executor
   thread just like sync blocking send.

7. **Deadlock freedom strengthened.**
   - Self-deadlock (I8): benign case (await reply from other pane)
     becomes possible without separate thread. Pathological case
     (request-from-self) still deadlocks, needs same prevention.
   - Cross-actor: [JHK24] Theorem 1.2 applies unchanged (topology).
   - New capability: multi-session interleaving via .await ([FH] KP4).
     Dependency cycles prevented by star topology acyclicity.

8. **Watchdog more natural.** Executor tick frequency IS the
   heartbeat. Current separate-thread design works unchanged or
   can be integrated into executor.

## One new risk

Multi-session interleaving: async handler can .await across
sessions, creating temporary dependency edges absent in sync.
Managed by star topology acyclicity ([JHK24] Theorem 1.2).
If Phase 2 adds pane-to-pane, re-examine regardless of model.

## What this supersedes

- research/async_session_bridge Option A recommendation (noop_waker)
  becomes unnecessary engineering with async looper
- rumpsteak_smol_translation Path A recommendation (verification
  only) upgradeable to Path B/C
