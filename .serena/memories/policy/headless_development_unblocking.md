---
type: policy
status: current
supersedes: [pane/headless_development_unblocking]
sources: [pane/headless_development_unblocking]
created: 2026-03-15
last_updated: 2026-04-11
importance: normal
keywords: [headless, development, workflow, pane_headless, compositor, default_target]
related: [decision/headless_strategic_priority]
agents: [pane-architect, all]
---

# Headless development unblocking

**Rule:** When planning work on any subsystem, default to
developing against `pane-headless`. Only pull in `pane-comp`
when the feature specifically requires rendering or input.

## Why

`pane-headless` eliminates the compositor as a development
bottleneck. Before headless, every subsystem required pane-comp
in a VM. Now:

- pane-roster, pane-store, pane-fs, scripting, AI kit,
  routing — all develop against pane-headless
- pane-shell's protocol side works headless; only rendering
  needs compositor
- The compositor becomes the last mile: chrome, input dispatch,
  layout, Wayland legacy

## How to apply

Default to headless. Pull in pane-comp only when the feature
specifically requires rendering or input. This rule supports
the broader strategic priority in
`decision/headless_strategic_priority`.
