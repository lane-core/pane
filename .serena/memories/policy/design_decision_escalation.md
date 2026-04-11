---
type: policy
status: current
supersedes: [pane/design_decision_escalation, auto-memory/feedback_design_decisions_require_input]
created: 2026-04-02
last_updated: 2026-04-10
importance: high
keywords: [design_decision, escalation, agent_consultation, lane_decides, autonomy_limits]
agents: [all]
---

# Design Decision Escalation

When a design question has multiple viable approaches, follow this
escalation path:

1. **Consult the four specialist agents** (be-systems-engineer,
   plan9-systems-engineer, session-type-consultant,
   optics-theorist) with the question and options. They may reach
   consensus that resolves it.
2. **If consensus resolves it**, apply and note the reasoning.
3. **If not**, forward the dilemma to Lane with the agents' input
   and the remaining options. Lane decides — either resolving it
   now or explicitly deferring as an open question.

Do NOT:

- Pick an option yourself based on "convention" or preference
- Skip the agent consultation step and go straight to Lane
- Reflexively file as an open question to avoid the collaborative
  step
- Move forward with implementation while noting "this is a design
  decision"

**Why:** Lane caught me (a) choosing option (a) over option (b)
for pane exit monitoring based on struck v1 precedent, (b)
defaulting to pane-native replacements for unix commands when the
design intent was additive enrichment, and (c) reflexively filing
the TLS→uid mapping as an open question instead of presenting it
for a decision. All three bypassed the collaborative step. The
agents exist to provide informed input; Lane exists to make the
call.

**How to apply:** Any question where two interpretations of the
spec lead to different implementations, any API surface decision,
any framing decision (replace vs enrich, explicit vs implicit),
and any case where the spec is silent on mechanism. "Convention"
from struck v1 code is not authority. The agents' domain expertise
is the first resource; Lane's judgment is the final authority.
