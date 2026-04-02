---
name: Pane-is-directory architecture
description: Clean-slate design — pane is a directory not a trait, services are opened handles not handler methods, protocol-level DeclareInterest unifies local/remote
type: project
---

Clean-slate architectural proposal (2026-03-31), framed as "strip away migration cost, argue design correctness only."

## Core decision

Pane stays a struct (the surface/directory). Handler stays a trait (compositor events only). Service capabilities (clipboard, observer, DnD) are opened at setup time as independent handles, each registering a calloop source. Opening a service = local calloop registration + protocol-level DeclareInterest.

## Key principles

1. **Opening a file is both capability acquisition and interest declaration.** `open_clipboard()` does calloop `insert_source()` AND sends `DeclareInterest` on the wire.
2. **Handler shrinks to compositor concerns.** Service methods (clipboard_lock_granted, etc.) move to callbacks on service handles.
3. **Service events flow through the unified batch.** Total ordering preserved. Filters see all events.
4. **No fd table, no generic ServiceHandle<T>.** Each service is concrete. calloop IS the multiplexer.
5. **Local = remote because open is a protocol operation.** Same code path over unix/TCP/TLS.

## Open questions

- Callback ergonomics: closures on handles vs handler dispatch gated by opened services
- Static (Pane) vs dynamic (Messenger) service opening — probably both
- Message enum still grows with service variants despite handler shrinkage

## Relationship to prior decisions

- Supersedes the "Pane-as-trait deferred" conclusion from handler_architecture_final
- Consistent with Phase 3 channel topology decisions (calloop as multiplexer, concrete not generic)
- Consistent with fail-at-use-site (Ehangup) service disconnect model

**Why:** Lane asked for clean-slate design reasoning, stripping away migration cost arguments.

**How to apply:** This is a proposed architecture, not a committed decision. Lane has not approved. Reference when Phase 3 implementation begins.
