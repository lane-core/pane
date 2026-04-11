---
type: reference
status: current
supersedes: [reference/haiku_book, auto-memory/reference_haiku_book]
sources: [reference/haiku_book, auto-memory/reference_haiku_book, .claude/agent-memory/be-systems-engineer/reference_haiku_book_local]
created: 2026-04-10
last_updated: 2026-04-10
importance: high
keywords: [haiku_book, doxygen, dox, BApplication, BHandler, BLooper, BMessenger, BMessage, BWindow]
related: [reference/haiku/_hub, reference/haiku/source]
agents: [be-systems-engineer, pane-architect]
---

# Haiku Book reference

The Haiku Book (Haiku's API reference documentation) is hosted
in-repo at `reference/haiku-book/`. 273 Doxygen `.dox` source
files, 3.7 MB, MIT licensed — copied from `haiku/haiku`
`docs/user/`.

Generate HTML: `cd reference/haiku-book && doxygen Doxyfile`

## Research protocol

When consulted on Be / Haiku API design, **read the actual `.dox`
files** in `reference/haiku-book/` rather than relying on recall.
Three-source triangulation:

1. **Book** (`reference/haiku-book/`) for the API contract
2. **Newsletter** (`~/src/haiku-website/`) for design rationale
3. **Source** (`~/src/haiku/`) for implementation details

## Key files for pane development

| pane concept | Haiku Book file |
|---|---|
| App lineage | `app/Application.dox` |
| Handler lineage | `app/Handler.dox` |
| Looper / threading model | `app/Looper.dox` |
| Messenger lineage | `app/Messenger.dox` |
| Message lineage | `app/Message.dox` |
| MessageFilter lineage | `app/MessageFilter.dox` |
| Pane (BWindow) lineage | `interface/Window.dox` |
| pane-notify lineage | `storage/NodeMonitor.dox` |

## Additional app kit docs

In `reference/haiku-book/app/`:

- `PropertyInfo.dox` — scripting property descriptors
- `Invoker.dox` — target / messenger binding
- `Clipboard.dox` — clipboard protocol
- `Roster.dox` — app roster / launch
- `MessageRunner.dox` — periodic message delivery
- `MessageQueue.dox` — message queue internals
- `_app_messaging.dox` — messaging overview / introduction

## Directory structure

Top-level dirs in `reference/haiku-book/`: `app`, `device`,
`drivers`, `game`, `graphics`, `interface`, `kernel`, `keyboard`,
`locale`, `mail`, `media`, `midi`, `net`, `storage`, `support`,
`translation`.

## Documentation style

`docs/kit-documentation-style.md` governs how pane's API docs are
written:

- Heritage annotations credit both Be and Haiku
- Design rationale belongs in doc comments, not external docs
- Rust doc conventions (`# Examples`, `# Panics`, etc.) required
- Match Be's tone (second-person, practical) but not Doxygen format
