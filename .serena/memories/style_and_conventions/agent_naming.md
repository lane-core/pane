# Agent Naming in Documentation and Examples

Use generic human names (ada, bob, ralph, etc.) for agent identities in docs, examples, and Display output — not dotted service-style names like `agent.reviewer`. 

**Why:** Dotted names are invalid unix usernames on most systems (periods not allowed in useradd on Linux, etc.). Since pane maps certificate subjects to local unix accounts, agent names must be valid usernames. Generic human names are both valid and readable.

**How to apply:** Anywhere an example agent identity appears — doc examples, Display impl output, test fixtures, architecture docs — use names like `ada`, `bob`, `ralph`. Never `agent.reviewer`, `agent.editor`, etc.
