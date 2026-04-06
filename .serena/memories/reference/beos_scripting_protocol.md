# BeOS Scripting Protocol Reference

Verified from Haiku source.

## ResolveSpecifier chain

- `BLooper::resolve_specifier()` (Looper.cpp:1428) — resolution loop. Iterates calling ResolveSpecifier() on current target, gets new target, repeats until target stabilizes or specifier stack exhausted.
- `BHandler::ResolveSpecifier()` (Handler.cpp:469) — base handler. Uses BPropertyInfo::FindMatch against sHandlerPropInfo (Suites, Messenger, InternalName).
- `BWindow::ResolveSpecifier()` (Window.cpp:2698) — resolves "View" to fTopView, "MenuBar" to fKeyMenuBar.
- `BView::ResolveSpecifier()` (View.cpp:5223) — resolves child views by index or name.

## Command constants (AppDefs.h:97-102)

B_GET_PROPERTY='PGET', B_SET_PROPERTY='PSET', B_CREATE_PROPERTY='PCRT', B_DELETE_PROPERTY='PDEL', B_COUNT_PROPERTIES='PCNT', B_EXECUTE_PROPERTY='PEXE'

## Specifier types (Message.h:42-49)

B_DIRECT_SPECIFIER=1, B_INDEX_SPECIFIER, B_REVERSE_INDEX_SPECIFIER, B_RANGE_SPECIFIER, B_REVERSE_RANGE_SPECIFIER, B_NAME_SPECIFIER, B_ID_SPECIFIER

## property_info (PropertyInfo.h:27-36)

name, commands[10], specifiers[10], usage, extra_data, types[10], ctypes[3] — the metadata that makes scripting discoverable.

## hey tool (hey.cpp)

Parses commands like `hey Tracker get Frame of Window 0` into scripting BMessages with specifier stacks.

## Mapping to pane optics

- DIRECT specifier → identity optic
- INDEX specifier → indexed lens
- NAME specifier → keyed lens
- ID specifier → identity-based lookup
- RANGE specifier → traversal
- Specifier stack = optic composition
- Hybrid approach: static optics within handlers, dynamic composition across handler boundaries