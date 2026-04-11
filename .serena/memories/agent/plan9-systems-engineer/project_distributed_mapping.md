---
name: pane distributed model mapping
description: Completed research mapping Plan 9's 9P, namespaces, import/exportfs, cpu, factotum, and plumber to pane's architecture — six areas with concrete recommendations
type: project
---

Research document at docs/superpowers/plan9-distributed-mapping.md. Completed 2026-03-30.

Six areas covered:
1. 9P -> pane-proto: keep typed protocol, adopt client-chosen IDs and explicit cancellation
2. Per-process namespaces -> graded equivalence: uid-based pane-fs views, .plan as namespace spec, no kernel namespaces
3. import/exportfs -> remote pane-fs: protocol bridge (not 9P mount), lazy connection, event forwarding
4. cpu -> remote execution: reverse connection model (remote app connects back to local compositor via TcpTransport+TLS), no namespace reconstruction needed
5. factotum -> .plan + TLS: do NOT build a factotum, use TLS + .plan + Landlock, Transport::peer_identity() trait method
6. plumber -> routing: kit-level distributed evaluation (not central server), user-editable rules, content transformation

**Why:** Lane is building headless/distributed deployment as the adoption path for pane (nix flake -> headless -> full Pane Linux).

**How to apply:** Reference this document when any distributed design question arises. The recommendations are concrete and the confidence levels are annotated.
