---
name: Filesystem scripting validation
description: Validated filesystem model vs BMessage scripting across 10 hey scenarios — 7 clean, 1 better, 2 need design attention; no dynamic Message escape hatch needed
type: project
---

Validated pane's architectural bet (filesystem at `/pane/<id>/` instead of dynamic BMessage fields) against 10 real BeOS scripting scenarios from hey.cpp and Haiku source.

**Result:** 7/10 clean, 1/10 better than BeOS (bulk command enumeration), 2/10 need design attention (per-app count, structured object creation).

**Why:** The filesystem model composes with unix tools and doesn't require a custom protocol. The friction points are (1) multi-property atomic operations (solved by structured `ctl` commands) and (2) per-application scoping (solved by `/pane/by-sig/` index or pane-store queries).

**Key decisions:**
- No `Message::Custom(BTreeMap<String, AttrValue>)` escape hatch needed
- `ctl` file needs defined command syntax (COMMAND [ARGS...])
- `attrs/` should support bulk-read (e.g., `/pane/1/attrs.json`)
- Per-signature index recommended (`/pane/by-sig/<signature>/`)
- Pane boundary as explicit design principle: no deep widget traversal, panes expose what they choose to expose

**How to apply:** When designing scripting/automation features, start from filesystem operations. Use `ctl` for commands, `attrs/` for state, `event` for monitoring. Don't add dynamic message fields to the protocol.

**Source files verified:**
- hey.cpp: verb parsing (get/set/do/create/delete/count), specifier chain construction, `with` clauses
- Application.cpp: Window/Looper resolution by index/name, B_COUNT_PROPERTIES for Window
- Window.cpp: property table (Active/Feel/Flags/Frame/Hidden/Look/Title/Workspaces/Minimize/TabFrame), ResolveSpecifier delegation to View/MenuBar
- View.cpp: property table (Frame/Hidden/View count/View traversal), child view resolution by index/name
- Menu.cpp: property table (Enabled/Label/Mark/Menu create-delete/MenuItem count-create-delete-execute), full CRUD for menu items
- TrackerScripting.cpp: Folder create/get/execute, Trash delete, Preferences execute
