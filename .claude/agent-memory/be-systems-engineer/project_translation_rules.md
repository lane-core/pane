---
name: Be-to-pane systematic translation rules
description: Complete decision tree for translating BeOS API concepts to pane-app, plus header audit, naming conventions, and inconsistency findings (2026-03-28)
type: project
---

Systematic translation rules codified (2026-03-28) from pane-app API iteration and full Haiku header audit.

**Why:** Reactive API decisions were correct individually but lacked a framework. This prevents drift and gives future decisions a consistent foundation.

**Decision tree (apply in order):**
0. Does it exist because of vertical integration? -> Identify the problem it solved, use pane's tools
1. Namespace mechanism (B prefix)? -> Drop it, crate path replaces it
2. Standalone type you construct and attach? -> Method on host, not standalone type
3. Dynamic typing Rust can express statically? -> Enum variants with typed fields
4. Virtual method override? -> Trait with default implementations, split per-variant
5. Lifecycle/ownership pattern? -> Value type consumed by run(), Messenger for ongoing communication
6. status_t / InitCheck? -> Result<T, E> from constructor
7. Observer/notification? -> Messenger::monitor() or pane-notify filesystem watches
8. System service? -> Separate crate or compositor service

**Naming conventions:**
- BFoo -> Foo (crate namespace replaces prefix)
- CamelCase methods -> snake_case
- Set/Get pairs -> set_xxx / bare getter (no get_ prefix)
- Is-predicates -> is_xxx
- B_CONSTANT_NAME -> Enum::Variant

**Inconsistencies found:**
1. Message::Close vs Handler::quit_requested (vocabulary mismatch — recommend CloseRequested or align both)
2. Message::Focus/Blur vs Handler::activated/deactivated (web vs Be terminology — recommend activated/deactivated throughout)
3. lib.rs doc example references removed Builtin type
4. event.rs/looper.rs doc comments still say "PaneEvent" instead of "Message"
5. App has no signature() getter (Be's BApplication::Signature() was public)
6. TimerToken is Clone but conceptually single-owner

**How to apply:** Use the decision tree for any new Be concept being adapted. Check the naming table for consistency. Fix the inconsistencies listed above.
