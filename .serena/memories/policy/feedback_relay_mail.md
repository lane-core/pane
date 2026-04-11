---
type: policy
status: current
supersedes: [auto-memory/feedback_relay_mail]
created: 2026-04-06
last_updated: 2026-04-10
importance: normal
keywords: [relay_mail, handoff_memo, agent_session, print_to_screen]
agents: [all]
---

# Relay mail workflow

**Rule:** Lane frequently asks for handoff mails to relay to other agent sessions — print to screen, Lane copies.

Lane commonly asks for "mail" to send to other agent sessions informing them of process changes, design decisions, or project state. The workflow is: print a formatted memo to the screen (To/From/Re header, concise bullet points), Lane copies it and pastes it into the next session.

**Why:** Agent sessions don't share context. Lane bridges them manually. The memo should be self-contained — the receiving agent has zero context about this session.

**How to apply:** When Lane asks to "make a mail" or "write a note for the next agent", produce a formatted memo with To/From/Re header, dated, covering what changed and why it matters for the receiving agent's work. Keep it concise but complete enough that the receiver can act on it without reading this session's history.
