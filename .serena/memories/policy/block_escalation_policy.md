---
type: policy
status: current
supersedes: [pane/block_escalation_policy, auto-memory/feedback_block_escalation_policy]
created: 2026-03-29
last_updated: 2026-04-10
importance: high
keywords: [block_escalation, blocks, design_decision, stop_work, silent_workaround]
agents: [all]
---

# Block Escalation Policy (CRITICAL)

If during implementation a block is encountered that would require
deviating from the originally expected specification or
agreed-upon plan:

1. **Stop work immediately.** Do not attempt to work around the
   block silently.

2. **Present what happened.** Describe the block concretely — what
   was attempted, what failed or proved impossible, what assumption
   was violated.

3. **Explain why it's a block.** Why the original strategy can't
   proceed as planned. What constraint or discovery changed the
   picture.

4. **Present options (if conceived).** For each option:
   - What the alternative approach is
   - What it changes about the plan
   - What consequences it has for downstream work
   - Whether it's a temporary workaround or a permanent design
     change

5. **Wait for direction.** Do not proceed until Lane decides which
   path to take.

**Why:** Silent workarounds accumulate into architectural drift. A
block that changes the implementation strategy is a design
decision, not an engineering detail. These decisions belong to
Lane, not to the agent. The cost of pausing to escalate is always
lower than the cost of discovering a silent deviation later.

**What counts as a block:** Anything that means the code will not
match what was agreed in the plan, spec, or conversation. Examples:
a library doesn't support what we assumed, a type system constraint
prevents the planned API shape, a performance requirement can't be
met with the chosen approach, a protocol design assumption is
wrong.

**What is NOT a block:** Normal debugging, compiler errors that
can be fixed without changing the approach, test failures from
implementation bugs, minor naming adjustments within the agreed
conventions.
