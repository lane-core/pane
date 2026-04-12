---
type: decision
status: decided
created: 2026-04-12
last_updated: 2026-04-12
importance: critical
keywords: [pane_session, MPST, EAct, par, session_types, NonBlockingSend, ActivePhase, flow_control, N1, N2, N3, N4, Fowler_Hu]
related: [decision/connection_source_design, decision/provider_side_api, agent/session-type-consultant/eact_mpst_pane_session_analysis, agent/pane-architect/mpst_paper_audit, status]
agents: [session-type-consultant, pane-architect, optics-theorist, plan9-systems-engineer, be-systems-engineer]
---

# pane-session MPST Foundation

Decided 2026-04-12 after adversarial testing exposed three bugs
(bidirectional buffer deadlock, partial-frame hang, head-of-line
blocking during queue fill) that are symptoms of dropping session
type discipline after the handshake. Lane identified that pane
has been handrolling mpsc channel code in pane-app instead of
building on par's session type runtime in pane-session. The
optics-theorist diagnosed the deadlock as a type error: ⊗
(unordered obligations) where ⅋ (sequenced obligations) is
required — session types prevent this by construction.

## Theoretical grounding

Fowler & Hu, "Speak Now: Safe Actor Programming with Multiparty
Session Types" (extended version) at
`~/gist/safe-actor-programming-with-multiparty-session-types/`.

pane IS an EAct system. The mapping is precise:
- Actor = Pane (Looper + Handler + ServiceDispatch)
- E-Send = send_request / send_notification (non-blocking)
- E-React = Handles<P>::receive (handler dispatch)
- E-Suspend = Handler returns Flow::Continue (implicit yield)
- E-RaiseS = panic + catch_unwind + Drop compensation
- E-CancelMsg = revoked_sessions + H3 suppression
- E-Monitor/E-InvokeM = Watch / PaneExited
- leave = ServiceHandle::Drop → RevokeInterest
- Forwarder = ProtocolServer ([CMS] §5.1)

## The gap

Par types the handshake: Send<Hello, Recv<Welcome>>. After
Welcome, session type discipline ENDS. The active phase is flat
enum dispatch — Handles<P>::receive(&mut self, msg) where every
variant is valid at every point. No CFSM state progression.

EAct's suspend(handler, state) installs a new handler with a
different type precondition at each step. pane re-enters the
same handler with the same type forever. Per-interaction linearity
IS enforced (ReplyPort exactly-once). Session-level state
progression is NOT.

## Four invariants (N1-N4)

Derived from EAct operational semantics. Make the three adversarial
bugs impossible by construction:

- **N1 (NonBlockingSend):** All looper-thread sends are
  non-blocking. Realizes [FH] E-Send. Prevents Bug 2
  (bidirectional deadlock).
- **N2 (Per-session credits):** Outstanding request counter per
  session_id. Realizes [GV10] bounded-buffer encoding. Prevents
  Bug 2 (bounded resource).
- **N3 (Failure cascade):** Transport error generates
  ServiceTeardown to all affected sessions. Realizes E-RaiseS +
  E-CancelMsg. Prevents Bug 1 (partial frame → clean teardown).
- **N4 (Frame atomicity):** FrameReader delivers only complete
  frames to the looper. Realizes E-Send atomic append. Prevents
  Bug 1 (no partial messages in session calculus).

## Architecture

```
par                → Handshake session types (Send/Recv/Dual)
pane-session       → Active phase session runtime (N1-N4)
pane-app           → Handler framework (dispatch, typed API)
```

Par stays for the handshake. pane-session builds the active-phase
runtime grounded in EAct.

## Extraction plan

Move from pane-app to pane-session:

| Abstraction | Source | What it provides |
|---|---|---|
| NonBlockingSend trait | implicit in D12 | Makes blocking sends unrepresentable (N1) |
| FlowControl | backpressure.rs | Per-session credit tracking (N2) |
| RequestCorrelator | dispatch.rs (token half) | Token alloc + matching, separated from handler closures |
| ActiveSession | implicit | Post-handshake state: params, session map, lifecycle |
| FrameReader | connection_source.rs | Non-blocking frame decoder (N4) |
| Failure cascade | scattered | Transport error → ServiceTeardown propagation (N3) |

pane-app keeps: DispatchEntry<H> with closures, ServiceHandle<P>,
obligation handles. These depend on handler type H.

## Global type (Scribble notation)

```
global protocol ServiceSession(
    role Consumer, role Server, role Provider
) {
  rec Loop {
    choice at Consumer {
      Request(Token, Payload) from Consumer to Server to Provider;
      choice at Provider {
        Reply(Token, Payload) from Provider to Server to Consumer;
      } or {
        Failed(Token) from Provider to Server to Consumer;
      }
      continue Loop;
    } or {
      Notification(Payload) from Consumer to Server to Provider;
      continue Loop;
    } or {
      Notification(Payload) from Provider to Server to Consumer;
      continue Loop;
    } or {
      Cancel(Token) from Consumer to Server to Provider;
      continue Loop;
    } or {
      RevokeInterest from Consumer to Server;
      ServiceTeardown from Server to Provider;
    }
  }
}
```

Server is forwarder per [CMS] §5.1. Load-bearing protocol is
Consumer↔Provider.

## Future: ActivePhase<T> typestate

ActivePhase<T> (already in PLAN.md) is the mechanism to close
the typestate gap. Generated CFSM state types from protocol
definitions, with per-state send/receive methods. Phase 2+ work,
dependent on ProtocolHandler derive macro and SessionEnum derive.

## Provenance

Lane caught that the three adversarial bugs (bidirectional
deadlock, partial-frame hang, HoL blocking) share one root cause:
dropping session type discipline after the handshake. Optics-
theorist diagnosed Bug 2 as ⊗ where ⅋ is required. Lane directed:
"figure out the underlying theoretical architecture of the system
we just implemented, and do it right from first principles
theoretically grounded in a session type formalism." Session-type
consultant mapped pane onto EAct, derived N1-N4. Pane-architect
audited the paper implementation and identified the typestate gap.

(Sources: `agent/session-type-consultant/eact_mpst_pane_session_analysis`,
`agent/pane-architect/mpst_paper_audit`,
`agent/optics-theorist/bidirectional_deadlock_analysis`,
Fowler & Hu "Speak Now" extended version.)
