---
type: decision
status: current
supersedes: [pane/headless_strategic_priority]
sources: [pane/headless_strategic_priority]
created: 2026-03-15
last_updated: 2026-04-11
importance: high
keywords: [headless, distributed, strategic_priority, funding, NLnet, pane_headless, deployment_model]
related: [decision/host_as_contingent_server, policy/headless_development_unblocking, decision/server_actor_model]
agents: [all]
---

# Headless pane strategic priority

Headless distributed pane is the top near-term deliverable.

## Why

1. **Funding pivot** — multi-architecture distributed pane is
   the best early path to funding.
2. **Proof of concept** — a running distributed system across
   heterogeneous hosts demonstrates design quality.
3. **Disciplining force** — designing for network transparency
   constrains architecture in ways that improve everything.
4. **Killer app without the OS** — users adopt pane via nix
   flake on existing systems (Darwin, Linux). Full Pane Linux
   is the next step, not the prerequisite.
5. **Incremental adoption** — flake config built during
   headless use IS the seed of the full system config. No cliff.

## How to apply

Every design decision should treat headless / distributed as
the **base case**. Docs should frame this as the foundational
deployment model. See `policy/headless_development_unblocking`
for the workflow consequence.
