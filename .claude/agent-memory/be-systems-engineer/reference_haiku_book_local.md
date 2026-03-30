---
name: Haiku Book local reference
description: Haiku Book API documentation (.dox files) is available locally at reference/haiku-book/ in the pane repo — primary reference for Be/Haiku API design questions
type: reference
---

The Haiku Book (273 Doxygen .dox files from haiku/haiku docs/user/, MIT licensed) is hosted locally in the pane repo at `reference/haiku-book/`.

## Research protocol update

When consulted about Be/Haiku API design, read the actual .dox files in `reference/haiku-book/` rather than relying on recall. Cross-reference with the Be Newsletter archive at `~/src/haiku-website/` for design rationale, and with Haiku source at `~/src/haiku/` for implementation details. Three-source triangulation: Book for API contract, Newsletter for "why", source for "how".

## Key files for pane's App Kit work

| pane concept | Haiku Book file | path |
|---|---|---|
| App lineage | Application.dox | `reference/haiku-book/app/Application.dox` |
| Handler lineage | Handler.dox | `reference/haiku-book/app/Handler.dox` |
| Looper/threading model | Looper.dox | `reference/haiku-book/app/Looper.dox` |
| Messenger lineage | Messenger.dox | `reference/haiku-book/app/Messenger.dox` |
| Message lineage | Message.dox | `reference/haiku-book/app/Message.dox` |
| MessageFilter lineage | MessageFilter.dox | `reference/haiku-book/app/MessageFilter.dox` |
| Pane (BWindow) lineage | Window.dox | `reference/haiku-book/interface/Window.dox` |

## Additional app kit docs in reference/haiku-book/app/

- `PropertyInfo.dox` — scripting property descriptors
- `Invoker.dox` — target/messenger binding
- `Clipboard.dox` — clipboard protocol
- `Roster.dox` — app roster / launch
- `MessageRunner.dox` — periodic message delivery
- `MessageQueue.dox` — message queue internals
- `_app_messaging.dox` — messaging overview/introduction

## Documentation style guide

`docs/kit-documentation-style.md` governs how pane's API docs are written. Key points:
- Heritage annotations credit both Be and Haiku
- Design rationale belongs in doc comments, not external docs
- Rust doc conventions (`# Examples`, `# Panics`, etc.) are required
- Match Be's tone (second-person, practical) but not Doxygen format

## Directory structure

Top-level dirs in `reference/haiku-book/`: app, device, drivers, game, graphics, interface, kernel, keyboard, locale, mail, media, midi, net, storage, support, translation (and build/config files).
