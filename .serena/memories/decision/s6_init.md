---
type: decision
status: current
supersedes: [pane/s6_init]
sources: [pane/s6_init]
created: 2026-03-15
last_updated: 2026-04-11
importance: normal
keywords: [s6, init, s6_rc, sixos, nix, pane_linux, distribution, service_management]
related: [decision/host_as_contingent_server]
agents: [pane-architect, plan9-systems-engineer]
---

# Init system: s6 (sixos)

pane's Linux distribution layer is planned to be based on
**sixos** (s6 + Nix):

- **s6** as init
- **s6-rc** for service management
- **Nix** for package management and system configuration

This is a distribution-layer decision, not a near-term
implementation commitment. The headless deployment model
(`decision/headless_strategic_priority`) does not require this
— users adopt pane via nix flake on existing systems first.
Full Pane Linux is the next step, not the prerequisite.
