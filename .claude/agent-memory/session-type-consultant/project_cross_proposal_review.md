---
name: Cross-proposal review (2026-04-01)
description: Session-type review of Be and Plan 9 greenfield proposals — agreements, disagreements, synthesis recommendation
type: project
---

Reviewed Be engineer and Plan 9 engineer greenfield architecture proposals against my own.

**All three agree on:** typed ingress/unified batch, Message::Clone elimination, commit()->Result, typestate handle preservation, pane-fs through protocol.

**Key disagreements:**

1. Both other proposals keep messaging methods (request_received, reply_received, reply_failed) on base Handler as "core." I argue these carry linear obligations (ReplyPort) and should follow the same extraction pattern as clipboard. Obligation-carrying methods don't belong on the base trait.

2. I proposed Sigma<H> — Plan 9 correctly rejected this as over-engineering. Rust match dispatch IS the defunctionalization. Conceded.

3. My proposal lost closure on-ramp (pane.run(closure)). Both others preserved it. Plan 9 handle model resolves this cleanly.

4. My proposal lacked protocol-level capability declaration (DeclareInterest). Plan 9's strongest unique contribution.

**Recommended synthesis:** Plan 9 handle-based service model + Be supertrait bounds for compile-time safety + obligation/event type-level distinction applied uniformly (including messaging, not just service protocols).

**How to apply:** Reference when finalizing Phase 3 architecture. The three proposals converge on mechanism (calloop multi-source) but diverge on API surface. This review identifies where each proposal's theory-grounded arguments are strongest.
