---
name: Optics doc audit 2026-04-03
description: Audit of optics-design-brief.md and scripting-optics-design.md against architecture.md — both stale, recommend archive
type: project
---

Both optics design docs are stale against docs/architecture.md. Key findings:

- **ScriptableHandler trait**: does not exist in architecture. `supported_properties()` is on Handler; specifier resolution belongs in Handles<Routing>.
- **ScriptQuery/ScriptResponse/ScriptError**: renamed to routing equivalents; routing queries go through Handles<Routing>::receive, not session-typed channels.
- **session_type! macro**: architecture uses #[derive(SessionEnum)] instead.
- **Message::ScriptQuery**: does not exist — Message is base-protocol only (resolved question 21).
- **FilterResult**: renamed to FilterAction.
- **send_and_wait in handlers**: prohibited by I8; optics docs assume it's available for script_get/script_set.
- **CompletionReplyPort, PropertyInfo, ghost state**: all absorbed into architecture.

**Why:** Lane requested audit to determine if docs should be archived. Both contain ~12-14 stale type references and 3-5 stale architectural assumptions each.

**How to apply:** Recommend both docs move to docs/archive/. The architecture spec is the sole source of truth for how optics integrate with the protocol. pane-optic crate rustdoc will be the living documentation for optic types.
