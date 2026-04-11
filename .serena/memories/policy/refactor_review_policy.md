---
type: policy
status: current
supersedes: [pane/refactor_review_policy, auto-memory/feedback_refactor_review_policy]
created: 2026-03-29
last_updated: 2026-04-10
importance: high
keywords: [refactor, review, code_review, stale_documentation, audit]
agents: [all]
---

# Post-Refactor Review Policy (CRITICAL)

After any substantial refactor (mass rename, API restructure,
architectural change):

1. **Code review** — audit the changed code for correctness,
   idiom, consistency
2. **Stale documentation review** (run in parallel with #1) —
   audit ALL comments, doc comments, specs, README, project docs,
   memory files for references to old names / patterns

If the code review produces actionable changes that are themselves
substantial (e.g., additional renames, structural fixes), then
after implementing those changes:

3. **Follow-up stale documentation review** — audit again for
   staleness introduced by the review fixes

The cycle repeats until a review pass produces no substantial
changes.

**Why:** We learned this the hard way — renaming PaneEvent →
Message → PaneMessage, PaneHandle → Messenger, etc. left dozens
of stale references in comments, specs, and docs that accumulated
across multiple rename rounds. Each round of fixes introduced new
staleness. The audit-after-refactor policy catches this
systematically.

**What counts as "substantial":** Any change that renames a public
identifier, removes / adds a public type, changes method
signatures, or restructures module organization. Single-line bug
fixes don't trigger this.
