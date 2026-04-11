---
type: forwarder
status: superseded
superseded_by: status
created: 2026-04-06
last_updated: 2026-04-10
---

# Forwarder: pane/current_state → status

This memory has moved. Read `status` instead:

```
mcp__serena__read_memory("status")
```

The original 2026-04-06 snapshot is preserved at
`archive/status/2026-04-06`.

This forwarder will be swept on the next migration cycle. New
references should use `status` directly.
