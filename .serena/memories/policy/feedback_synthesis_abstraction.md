---
type: policy
status: current
supersedes: [pane/feedback_synthesis_abstraction, auto-memory/feedback_synthesis_abstraction]
created: 2026-03-18
last_updated: 2026-04-10
importance: normal
keywords: [synthesis, abstraction_level, research, philosophical_ideas, granular_features]
agents: [all]
---

# Synthesis at the Right Abstraction Level

**Rule:** Research synthesis should focus on philosophical ideas
and design principles, not granular feature mappings between
reference systems and pane.

When synthesizing how reference systems (BeOS, Plan 9, session
types) inform pane's design, focus on the philosophical ideas —
why protocol discipline produces stability, why interface
uniformity enables emergent composition — not on mapping specific
features ("Plan 9 has X, pane has Y"). Granular design choices
(cells, JSON formats, file layouts) are misleading at this stage
because they suggest decisions that could change. The potent
content is the principles that clarify intention.

**Why:** Pane's broad outlines and intended function are clear
from the README, architecture spec, and design pillars — evocative
reference points that give the general shape. What's not settled
is the granular implementation layer. The research and
spec-tightening clarify the endeavor by grounding it in genuine
understanding of the references. Synthesis that maps
reference-system mechanisms to specific pane implementation
details creates false precision at a level that isn't decided yet.

**How to apply:** Write at the level of ideas and what they mean
for a system with pane's stated intentions. The broad design is
known — small servers, typed protocols, filesystem interfaces,
text-as-action. The research deepens understanding of *why* these
ideas work and where the philosophical commitments lead. Don't
presume granular design choices, but don't pretend the shape is
unknown either. Right level: "protocol discipline produces
stability, and here's what that means for pane's approach" — not
"BMessage maps to PaneMessage" and not "we have no idea what pane
is."
