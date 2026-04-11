---
type: decision
status: current
supersedes: [pane/system_fonts]
sources: [pane/system_fonts]
created: 2026-03-25
last_updated: 2026-04-11
importance: low
keywords: [system_fonts, inter, gelasio, monoid, sans, serif, monospace, glyph_atlas, compositor]
related: []
agents: [pane-architect]
---

# pane system default fonts

Three default fonts for the compositor and applications:

- **Inter** — UI sans-serif. Used for window chrome (tag titles,
  button labels, command surface text), system UI, and as the
  default proportional font.
- **Gelasio** — Serif font. Available as the system serif option.
- **Monoid** — Monospace font. Used for terminal content, code
  display, and as the default fixed-width font. **This is what
  the glyph atlas should load by default.**

These are the system defaults — applications may use other fonts,
but these are what pane ships with and what the compositor uses
for its own rendering.
