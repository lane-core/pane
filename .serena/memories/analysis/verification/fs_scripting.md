# Filesystem Scripting Validation

Validated pane's filesystem model (`/pane/<id>/`) against 10 real BeOS scripting scenarios from hey.cpp and Haiku source.

**Result:** 7/10 clean, 1/10 better than BeOS (bulk command enumeration), 2/10 need design attention (per-app count, structured object creation).

## Key decisions

- No `Message::Custom(BTreeMap<String, AttrValue>)` escape hatch needed
- `ctl` file needs defined command syntax (COMMAND [ARGS...])
- `attrs/` should support bulk-read (e.g., `/pane/1/attrs.json`)
- Per-signature index recommended (`/pane/by-sig/<signature>/`)
- Pane boundary as explicit design principle: no deep widget traversal, panes expose what they choose to expose

## Friction points

1. Multi-property atomic operations → solved by structured `ctl` commands
2. Per-application scoping → solved by `/pane/by-sig/` index or pane-store queries

**How to apply:** When designing scripting/automation features, start from filesystem operations. Use `ctl` for commands, `attrs/` for state, `event` for monitoring. Don't add dynamic message fields to the protocol.

**Sources verified:** hey.cpp, Application.cpp, Window.cpp, View.cpp, Menu.cpp, TrackerScripting.cpp