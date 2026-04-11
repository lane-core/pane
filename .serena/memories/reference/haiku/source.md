---
type: reference
status: current
sources: [agent/be-systems-engineer/reference_haiku_source]
created: 2026-04-10
last_updated: 2026-04-10
importance: normal
keywords: [haiku_source, file_paths, BLooper, BHandler, MessageLooper, ServerApp, ServerWindow, scripting, translation_kit]
related: [reference/haiku/_hub, reference/haiku/book, policy/heritage_annotations]
agents: [be-systems-engineer, pane-architect]
---

# Haiku source layout

Key file paths in `~/src/haiku/` for verifying BeOS architecture
claims. Use these for source citations in heritage annotations
(see `policy/heritage_annotations`).

## Messaging primitives

- `headers/os/app/Looper.h` — BLooper: thread with message queue, recursive locking via sem
- `headers/os/app/Handler.h` — BHandler: MessageReceived, ResolveSpecifier (scripting), observer pattern (StartWatching / SendNotices)
- `headers/os/app/Message.h` — BMessage: typed name-value pairs, scripting specifiers
- `headers/os/app/MessageFilter.h` — cross-cutting message interception
- `headers/os/app/PropertyInfo.h` — `property_info` struct: scripting protocol metadata (commands, specifiers, types, compound_types)

## app_server threading model

- `src/servers/app/MessageLooper.h` — server-side thread abstraction (thread_id, port_id, _MessageLooper loop)
- `src/servers/app/ServerApp.h` — per-application server-side object, extends MessageLooper
- `src/servers/app/ServerWindow.h` — per-window server-side object, extends MessageLooper

Both `ServerApp` and `ServerWindow` have their own threads
(MessageLooper pattern).

## Scripting protocol

- `BHandler::ResolveSpecifier` and `GetSupportedSuites` are on every BHandler
- `property_info` in `PropertyInfo.h` defines `commands[10]`, `specifiers[10]`, `types[10]`
- Specifiers: `B_DIRECT_SPECIFIER`, `B_INDEX_SPECIFIER`, `B_NAME_SPECIFIER`, `B_ID_SPECIFIER`, etc.
- `AppDefs.h`: `B_GET_PROPERTY='PGET'`, `B_SET_PROPERTY='PSET'`

See `reference/haiku/scripting_protocol` for the full mechanics.

## Translation Kit

- `headers/os/translation/TranslationDefs.h` — `translation_format` with quality / capability floats
- `headers/os/translation/Translator.h` — BTranslator base class
- `headers/os/translation/TranslatorRoster.h` — discovery and mediation

## Confirmed inheritance

`BWindow` inherits `BLooper` — confirmed in
`headers/os/interface/Window.h:93`.
