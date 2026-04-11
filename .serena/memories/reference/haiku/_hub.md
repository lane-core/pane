---
type: reference
status: current
created: 2026-04-10
last_updated: 2026-04-10
importance: high
keywords: [haiku, beos, haiku_book, app_server, scripting, BLooper, BHandler, hub]
related: [policy/beapi_translation_rules, policy/beapi_naming_policy]
sources: []
verified_against: [pane/.serena/memories/reference, pane/.claude/agent-memory/be-systems-engineer]
agents: [be-systems-engineer, pane-architect]
---

# Haiku / BeOS reference

Canonical reference for "what did Be do?" and "how does Haiku
do it?" questions. The local Haiku Book copy lives at
`reference/haiku-book/` in the pane repo (273 Doxygen `.dox`
files, MIT licensed). Haiku source at `~/src/haiku/`. Be
Newsletter archive at `~/src/haiku-website/`.

## Spokes

- [`reference/haiku/book`](book.md) — Haiku Book contents and which `.dox` files matter for pane
- [`reference/haiku/source`](source.md) — where to find specific subsystems in `~/src/haiku/`
- [`reference/haiku/haiku_rs`](haiku_rs.md) — haiku-rs Rust bindings (FFI, not a reimplementation)
- [`reference/haiku/scripting_protocol`](scripting_protocol.md) — ResolveSpecifier chain, specifier types, property_info, hey
- [`reference/haiku/naming_philosophy`](naming_philosophy.md) — Be's naming conventions verified from Haiku headers
- [`reference/haiku/appserver_concurrency`](appserver_concurrency.md) — app_server's per-client port + MultiLocker model
- [`reference/haiku/decorator_architecture`](decorator_architecture.md) — Haiku's window decorator system
- [`reference/haiku/internals`](internals.md) — BLooper lock contention, BMessage internals, BClipboard, BRoster
- [`reference/haiku/beapi_divergences`](beapi_divergences.md) — pane's tracker for every Be type → pane type with rationale

## Where the rules live

- `policy/beapi_naming_policy` — three-tier Be naming policy
- `policy/beapi_translation_rules` — systematic Be → pane translation rules
- `policy/heritage_annotations` — citation format for Be / Plan 9 in Rust doc comments

## When to consult

- API design questions ("what would BLooper do?") → start here, descend to the relevant spoke
- Heritage citations in Rust doc comments → `reference/haiku/source` for paths
- "Why does pane diverge from Be on X?" → `reference/haiku/beapi_divergences`
- Newcomer orientation → read this hub, then `book`, then `internals`
