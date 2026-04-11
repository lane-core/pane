---
type: reference
status: current
created: 2026-04-10
last_updated: 2026-04-10
importance: high
keywords: [plan9, 9P, namespace, cs, plumber, factotum, rio, exportfs, hub]
related: [policy/technical_writing, policy/heritage_annotations, reference/haiku/_hub]
agents: [plan9-systems-engineer, pane-architect, session-type-consultant]
---

# Plan 9 reference

The Plan 9 design is the second major influence on pane (alongside
BeOS / Haiku). Local references:

- **The 1995 paper** at `~/gist/plan9.pdf` (Pike et al.,
  *Computing Systems* 8(3))
- **First Edition Programmer's Manual** (1993):
  <https://doc.cat-v.org/plan_9/1st_edition/manual.pdf>
- **Web man pages**: <https://9p.io/sys/man/>
- **Vendored material** under `reference/plan9/` (papers, man
  pages by section, source excerpts)

## Spokes

- [`reference/plan9/foundational`](foundational.md) — the Pike 1995 paper, three design principles, key insights, what they'd do differently
- [`reference/plan9/voice`](voice.md) — writing voice analysis from 12 Plan 9 papers (the three-tier model used by `policy/technical_writing`)
- [`reference/plan9/papers_insights`](papers_insights.md) — technical insights from 12 vendored papers (8½, plumb, auth, net, names, rc, mk, comp, sleep, acme, compiler, namespace)
- [`reference/plan9/man_pages_insights`](man_pages_insights.md) — primary-source findings from rio(4), plumber(4), proc(3), srv(3), factotum(4), namespace(4), thread(2), wait(2)
- [`reference/plan9/distribution_model`](distribution_model.md) — Phase 2 distribution design (remote pane-fs, cpu mapping, import / exportfs)
- [`reference/plan9/divergences`](divergences.md) — every Plan 9 concept and how pane adapts it (the long tracker)
- [`reference/plan9/decisions`](decisions.md) — short reference of adopted / adapted / rejected patterns

## Where the rules live

- `policy/technical_writing` — Plan 9 voice for pane docs (cites `voice` and `foundational`)
- `policy/heritage_annotations` — citation format for Plan 9 in Rust doc comments

## When to consult

- "What does plumber do?" → `man_pages_insights`
- "How should pane handle the Plan 9 voice in docs?" → `voice`, then `policy/technical_writing`
- "Why did pane do X differently from Plan 9?" → `divergences`
- "Was Plan 9 X adopted, adapted, or rejected?" → `decisions` (short summary)
- "What's pane's Phase 2 distribution design?" → `distribution_model`
- Reading the Pike paper for the first time → `foundational`
