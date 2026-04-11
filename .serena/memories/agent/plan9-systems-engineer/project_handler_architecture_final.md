---
name: Handler architecture final assessment
description: Three-way debate concluded — unified Handler + callback-struct extension, protocol-level capabilities, deferred trait split (2026-03-31)
type: project
---

Final assessment after three-round debate between Plan 9, Be, and session-type positions (2026-03-31).

## Settled decisions

1. **Typed ingress, unified batch.** Per-service calloop sources convert to Message at callback boundary, flow through unified coalesce/filter/dispatch. All three positions agreed.
2. **Handler stays unified.** No trait split. Service-specific callbacks live on service objects (ClipboardCallbacks on Clipboard, etc.).
3. **Protocol-level capability declaration.** Extend ClientCaps with service interest. Compositor routes only declared services. Load-bearing for distribution.
4. **Fail-at-use-site for service disconnects.** Ehangup model. commit() returns Result. No Handler methods per service disconnect.
5. **No fd table / generic ServicePane<S>.** calloop IS the multiplexer. Build concrete, extract after second service.
6. **Pane-as-trait deferred.** Looper specialization unsolved. Callback-struct may prove sufficient.

## Required changes before Phase 3

- Fix Message::Clone (panics on AppMessage/Reply/CompletionRequest/ClipboardLockGranted)
- ClipboardWriteLock::commit() returns Result
- Three clipboard methods migrate to ClipboardCallbacks struct pattern

## Open risks

- Handler method count growth across 5+ services (may force reconsideration)
- Per-service reconnection ordering edge cases
- Looper routing decision for callback-struct vs Handler method dispatch

**Why:** Lane asked for comprehensive final assessment to guide Phase 3 implementation.

**How to apply:** Reference when implementing clipboard channel in Phase 3. Key constraint: service interest must be in protocol handshake, not just local trait bounds. Callback-struct pattern before trait split.
