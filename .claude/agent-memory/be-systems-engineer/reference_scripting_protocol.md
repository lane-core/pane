---
name: BeOS scripting protocol implementation details
description: Detailed scripting protocol mechanics verified from Haiku source — ResolveSpecifier chain, specifier types, property_info, hey tool — and the mapping to pane's optics framework
type: reference
---

**The ResolveSpecifier chain (verified in Haiku source):**

- `BLooper::resolve_specifier()` at `src/kits/app/Looper.cpp:1428` — the resolution loop. Iterates calling `ResolveSpecifier()` on current target, gets new target, repeats until target stabilizes or specifier stack exhausted.
- `BHandler::ResolveSpecifier()` at `src/kits/app/Handler.cpp:469` — base handler. Uses BPropertyInfo::FindMatch against sHandlerPropInfo (Suites, Messenger, InternalName properties).
- `BWindow::ResolveSpecifier()` at `src/kits/interface/Window.cpp:2698` — resolves "View" property to fTopView, "MenuBar" to fKeyMenuBar.
- `BView::ResolveSpecifier()` at `src/kits/interface/View.cpp:5223` — resolves child views by index or name.

**Command constants** (`headers/os/app/AppDefs.h:97-102`):
- B_GET_PROPERTY='PGET', B_SET_PROPERTY='PSET', B_CREATE_PROPERTY='PCRT'
- B_DELETE_PROPERTY='PDEL', B_COUNT_PROPERTIES='PCNT', B_EXECUTE_PROPERTY='PEXE'

**Specifier types** (`headers/os/app/Message.h:42-49`):
- B_DIRECT_SPECIFIER=1, B_INDEX_SPECIFIER, B_REVERSE_INDEX_SPECIFIER
- B_RANGE_SPECIFIER, B_REVERSE_RANGE_SPECIFIER, B_NAME_SPECIFIER, B_ID_SPECIFIER

**property_info** (`headers/os/app/PropertyInfo.h:27-36`):
- name, commands[10], specifiers[10], usage, extra_data, types[10], ctypes[3]
- The metadata that makes scripting discoverable

**The hey tool** (`src/bin/hey.cpp`) — parses commands like `hey Tracker get Frame of Window 0` into scripting BMessages with specifier stacks.

**Mapping to pane optics (proposed in architecture review):**
- DIRECT specifier -> identity optic
- INDEX specifier -> indexed lens
- NAME specifier -> keyed lens
- ID specifier -> identity-based lookup
- RANGE specifier -> traversal
- Specifier stack = optic composition
- Recommendation: hybrid approach — static optics within handlers, dynamic composition across handler boundaries
