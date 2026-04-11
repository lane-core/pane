---
type: policy
status: current
supersedes: [style_and_conventions/agent_naming]
created: 2026-03-20
last_updated: 2026-04-10
importance: normal
keywords: [agent_naming, examples, ada, bob, ralph, unix_username, certificate_subject]
agents: [pane-architect]
---

# Agent Naming in Documentation and Examples

**Rule:** Use generic human names (ada, bob, ralph, etc.) for agent identities in docs, examples, and Display output — not dotted service-style names like `agent.reviewer`.

**Why:** Dotted names are invalid unix usernames on most systems (periods not allowed in useradd on Linux, etc.). Since pane maps certificate subjects to local unix accounts, agent names must be valid usernames. Generic human names are both valid and readable.

**How to apply:** Anywhere an example agent identity appears — doc examples, Display impl output, test fixtures, architecture docs — use names like `ada`, `bob`, `ralph`. Never `agent.reviewer`, `agent.editor`, etc.
