---
type: reference
status: current
supersedes: [reference/beos_scripting_protocol]
sources: [reference/beos_scripting_protocol, .claude/agent-memory/be-systems-engineer/reference_scripting_protocol]
created: 2026-04-10
last_updated: 2026-04-10
importance: high
keywords: [scripting, ResolveSpecifier, BHandler, BLooper, BWindow, BView, property_info, hey, specifier, optic_mapping]
related: [reference/haiku/_hub, reference/haiku/source, reference/haiku/internals]
agents: [be-systems-engineer, optics-theorist, pane-architect]
---

# BeOS scripting protocol

Verified from Haiku source. The scripting protocol is BeOS's
introspection / remote-control mechanism — every BHandler can
declare properties and respond to specifier-stack queries.

## ResolveSpecifier chain

- `BLooper::resolve_specifier()` (`src/kits/app/Looper.cpp:1428`)
  — the resolution loop. Iterates calling `ResolveSpecifier()`
  on the current target, gets a new target, repeats until target
  stabilizes or specifier stack exhausted.
- `BHandler::ResolveSpecifier()` (`src/kits/app/Handler.cpp:469`)
  — base handler. Uses `BPropertyInfo::FindMatch` against
  `sHandlerPropInfo` (Suites, Messenger, InternalName).
- `BWindow::ResolveSpecifier()`
  (`src/kits/interface/Window.cpp:2698`) — resolves "View" to
  `fTopView`, "MenuBar" to `fKeyMenuBar`.
- `BView::ResolveSpecifier()` (`src/kits/interface/View.cpp:5223`)
  — resolves child views by index or name.

## Command constants (`headers/os/app/AppDefs.h:97-102`)

- `B_GET_PROPERTY='PGET'`, `B_SET_PROPERTY='PSET'`
- `B_CREATE_PROPERTY='PCRT'`, `B_DELETE_PROPERTY='PDEL'`
- `B_COUNT_PROPERTIES='PCNT'`, `B_EXECUTE_PROPERTY='PEXE'`

## Specifier types (`headers/os/app/Message.h:42-49`)

- `B_DIRECT_SPECIFIER=1`, `B_INDEX_SPECIFIER`,
  `B_REVERSE_INDEX_SPECIFIER`
- `B_RANGE_SPECIFIER`, `B_REVERSE_RANGE_SPECIFIER`,
  `B_NAME_SPECIFIER`, `B_ID_SPECIFIER`

## property_info (`headers/os/app/PropertyInfo.h:27-36`)

`name`, `commands[10]`, `specifiers[10]`, `usage`, `extra_data`,
`types[10]`, `ctypes[3]` — the metadata that makes scripting
discoverable.

## hey tool

`src/bin/hey.cpp` — parses commands like
`hey Tracker get Frame of Window 0` into scripting BMessages
with specifier stacks.

## Mapping to pane optics

- DIRECT specifier → identity optic
- INDEX specifier → indexed lens
- NAME specifier → keyed lens
- ID specifier → identity-based lookup
- RANGE specifier → traversal
- Specifier stack = optic composition

Hybrid approach (proposed in architecture review): static optics
within handlers, dynamic composition across handler boundaries.
