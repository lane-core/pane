---
name: Service disconnect model — fail at use site
description: Clipboard (and future services) surface disconnection through Result errors on operations, not proactive handler notification
type: project
---

Consultation on clipboard service disconnect handling (2026-03-31).

## Decision: Option 3 — fail at use site, no proactive notification

Three options were evaluated for surfacing clipboard service disconnect to the handler:
1. Dedicated `Message::ClipboardServiceDisconnected` + handler method — rejected
2. Generic `AppMessage` delivery with downcast — rejected
3. Silent handling: operations return `Err`, no proactive notification — **adopted**

## Rationale (Plan 9 model)

Plan 9 mount disconnection: next walk/read/write returns Ehangup. No signal, no async notification. Process discovers failure at the point of use. This works because:
- Clipboard operations already return `Result` — disconnection is just another error variant
- Clipboard access is rare/event-driven — proactive notification mostly reaches handlers that don't care
- Terminal disconnects (compositor) and non-terminal disconnects (services) should use different mechanisms; compositor already has `Message::Disconnected` → `ExitReason::Disconnected`
- Scales to N services without growing Handler trait per service

## One refinement

`ClipboardWriteLock::commit()` should return `Result<(), CommitFailed>` instead of `()`. Currently the send failure is silently ignored via `let _`. This is the one path where fail-at-use-site needs an API change.

## Scaling principle

5 services, 5 mount points — same answer. Dedicated disconnect methods grow Handler linearly. Generic `service_disconnected(name)` reinvents SIGPIPE. Fail-at-use-site adds zero API surface per service.

Exception: push-based streams (e.g., clipboard watch delivering ClipboardChanged). When the stream stops, the handler just stops receiving events. If explicit end-of-stream is ever needed, use an Event::End sentinel on the stream, not a separate disconnect handler.

**Why:** Lane asked for Plan 9 perspective on how to surface non-terminal service disconnects.

**How to apply:** When implementing clipboard channel in Phase 3, ensure all clipboard API methods return `Result`. Change `commit()` return type. Do not add disconnect variants to Message or methods to Handler for services.
