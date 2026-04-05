---
name: EAct framework architecture assessment
description: Assessment (2026-04-03) that pane-app IS an EAct framework using par CLL channels — heritage fidelity, par as substrate, EAct correspondence gaps, MPST not needed
type: project
---

Assessed the proposal: par (CLL binary) + pane-session (IPC bridge) + pane-app (EAct framework). Conclusion: the three-tier stack is the right architecture. No telltale/MPST needed.

**Heritage fidelity:** BLooper was an actor (one thread, one queue, sequential dispatch). BHandler chain = handler store sigma. BWindow had two channels to app_server (fLink for sync, fMsgPort for async events) — multi-channel composition on a single actor. Framing pane-app as EAct is faithful and illuminating. Be's architecture was correct intuitively; EAct proves why.

**par as substrate:** Keep reimplementation, don't take crate dependency. par panics on disconnect (in-process assumption); pane-session's crash safety (Result not panic) is fundamental. par's type vocabulary (Send/Recv/Select/Branch/End) is sufficient. pane-session needs Queue for active-phase streaming but NOT par's Server type (calloop handles that). SessionEnum (N-ary branching) is pane's genuine contribution beyond par.

**EAct correspondence:** Handler = actor (tight). Handles<P> + Dispatch<H> = sigma (tight). send_request = E-Suspend, reply dispatch = E-React (tight). Gaps: (1) no E-New equivalent for mid-session service hot-plug, (2) multi-session interleaving argument implicit not explicit in docs, (3) PaneBuilder restricts sigma growth more than EAct allows (intentional, buys compile-time checking). Pane implements *restricted* EAct: static service binding + dynamic request correlation.

**MPST not needed:** BeOS never had global protocol specs. Each server relationship was bilateral. EAct's Progress theorem (correct binary sessions + reactive actors = system progress) is the right assurance level for a desktop. MPST scales with composition complexity; desktop has shallow interaction depth (2-3 hops, 6-8 service types). The structural argument is tractable.

**Key BWindow detail:** Window.cpp line 761 — BWindow passes both fLink->ReceiverPort() and fMsgPort to the app_server. Two channels, one thread. This is the multi-session composition pattern that pane's typed ingress (per-protocol calloop channels) formalizes.

**Why:** This assessment resolves the three-tier vs MPST question. The bottom-up composition model (par channels + EAct actors) matches Be's heritage and is sufficient for desktop protocol composition.

**How to apply:** When implementing pane-app, frame it explicitly as EAct. When documenting, make the multi-session interleaving argument explicit. When Phase 3 introduces service hot-plug, that's where E-New semantics need design work.
