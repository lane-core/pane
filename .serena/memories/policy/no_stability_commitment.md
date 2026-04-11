---
type: policy
status: current
supersedes: [pane/no_stability_commitment, auto-memory/feedback_no_deprecations]
created: 2026-03-31
last_updated: 2026-04-10
importance: high
keywords: [no_stability, no_deprecations, pre_stable, rename_freely, remove_dead_code]
agents: [all]
---

# No Stability Commitment

pane has no users. The API and architecture are free to change
without deprecation, migration paths, or backwards compatibility.
There is no commitment to any stability in the API or architecture
as a whole.

## Consequences

- **Remove dead code outright, don't deprecate.** No
  `#[deprecated]` attribute. When a method is superseded (e.g.,
  `send_periodic` replaced by `send_periodic_fn`), remove the old
  method, update all internal callers, and move on. Same for dead
  error variants, unused type aliases, etc.
- **Rename freely** when a better name is found
- **Restructure types and traits** without shims
- **Don't write migration guides** or upgrade documentation
- **Don't hedge design decisions** around "breaking changes" —
  there are no downstream consumers to break

**Why:** Deprecation warnings are noise when there are no
downstream users to migrate. The `#[deprecated]` attribute is for
published APIs with consumers who need a migration path. pane has
neither.

Lane will inform when this changes.
