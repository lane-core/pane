---
type: analysis
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: critical
keywords: [eact, mpst, pane-session, global-type, multiplexer, active-phase, forwarder, non-blocking-send, three-bugs, par-scope]
sources:
  - "[FH] §2 KP1-KP5, §3.2 E-Send/E-React/E-Suspend, §3.3 Lemma 1 (Independence), Theorem 5 (Progress), Corollary 1 (Global Progress), §4 E-RaiseS/E-CancelMsg/E-CancelH/E-InvokeM, §5 Theorems 6-8, Discussion (leave construct)"
  - "[JHK24] Theorem 1.2 (star topology progress)"
  - "[CMS] §5.1 (forwarder chain cut-elimination)"
  - "[TCP11] §3 (dependent continuation types)"
  - "[HYC08] (MPST projection)"
  - "[SY19] (generalized MPST compliance)"
  - "[GV10] §5 (bounded-buffer in session types)"
  - "par 0.3.10 exchange.rs:23 (oneshot), server.rs:128 (rendezvous)"
  - "decision/connection_source_design D1-D12"
  - "architecture/session, architecture/app, architecture/looper"
verified_against:
  - "eventactors-extended.tex lines 1900-2450 (operational semantics)"
  - "eventactors-extended.tex lines 2777-3150 (metatheory)"
  - "eventactors-extended.tex lines 3153-3900 (failure handling)"
  - "eventactors-extended.tex lines 4484-4531 (chat server protocol)"
  - "connection_source.rs:1-100 (FrameReader)"
  - "server.rs:1-80 (ProtocolServer actor)"
  - "subscriber_sender.rs:1-50 (SubscriberSender)"
  - "backpressure.rs:1-50 (Backpressure enum)"
related: [reference/papers/eact, reference/papers/dlfactris, reference/papers/forwarders, reference/papers/dependent_session_types, analysis/eact/_hub, analysis/session_types/_hub, decision/server_actor_model, decision/connection_source_design, agent/session-type-consultant/active_phase_session_analysis, agent/session-type-consultant/stress_test_bug_analysis]
agents: [session-type-consultant]
---

# EAct MPST → pane-session Analysis

Lane decided: Option B — build a session-typed multiplexer on par,
grounded in multiparty session type theory. This analysis maps
pane's architecture onto the EAct formalism, defines the global
type, specifies what pane-session should provide, shows how the
three adversarial bugs become impossible by construction, and
evaluates par's scope.

## 1. Architecture mapping

- Pane = EAct actor (4-tuple: Address, looper state, ServiceDispatch, PaneBuilder init)
- Service binding = EAct session (session_id = session name s)
- ProtocolServer = runtime infrastructure + forwarder ([CMS] §5.1)
- DeclareInterest = E-Register + E-Init (bilateral specialization)
- Handles<P>::receive = handler value (TV-Handler), dispatch = E-React
- send_notification/send_request = E-Send (non-blocking queue append)
- Watch/PaneExited = E-Monitor/E-InvokeM (one-shot)
- Handler panic cascade = E-RaiseS + E-CancelMsg + E-CancelH
- RevokeInterest = leave construct ([FH] §5 Discussion)

## 2. Global type

Tripartite: Consumer ↔ Server ↔ Provider. Server is a forwarder.
ServiceSession has recursive choice: Request/Reply, Notification
(both directions), Cancel, RevokeInterest, ServiceTeardown.
Out-of-order replies via Token correlation = dependent session
([TCP11] §3). Binary projection to two bilateral sessions is
sound per [HYC08] projection + [CMS] forwarder chain.

## 3. pane-session must provide

1. ActiveSession — post-handshake multiplexer state (HashMap<u16, SessionState>)
2. NonBlockingSend trait — type-level enforcement of [FH] E-Send non-blocking
3. Per-session flow control — credit caps from [GV10] §5 bounded-buffer encoding
4. Failure cascade — E-RaiseS/E-CancelMsg detection + ServiceTeardown generation
5. Session lifecycle — DeclareInterest/InterestAccepted/RevokeInterest/Cancel routing

## 4. Three bugs → impossible by construction

- Bug 1 (partial frame): FrameReader WouldBlock state machine = atomic E-Send postcondition. Looper never sees partial frames.
- Bug 2 (deadlock): NonBlockingSend + D12 SharedWriter = [FH] Lemma 1 (Independence). No blocking send ⇒ no deadlock cycle.
- Bug 3 (HoL): D12 per-connection writer threads = per-session queue independence. [FH] Lemma 1 applies to writer threads.

## 5. Par's scope

Par stays for handshake (binary, well-typed, correct). Active
phase is genuinely multiplexed, non-deterministic, dependent —
par's sequential binary types cannot express it. pane-session
builds its own session runtime grounded in EAct formalism.

## Verdict

Conditionally sound. EAct Theorems 4-8 transfer to pane under
existing conditions (I2/I3 watchdog, I6 single-mailbox, affine-
with-compensation). The new pane-session runtime must enforce:
(N1) NonBlockingSend for all looper-thread sends,
(N2) per-session credit tracking,
(N3) failure cascade generating ServiceTeardown on any transport error,
(N4) FrameReader atomicity (complete frames only to looper).
These four invariants are the pane-specific realization of the
EAct operational semantics.
