# EAct Concepts NOT Appropriate for Pane

Aspects of Fowler and Hu's EAct calculus ("Speak Now") that should NOT be adopted, with rationale. Knowing what to avoid is as important as knowing what to adopt.

## Flow-Sensitive Effect System

EAct annotates function arrows with pre/post session-type conditions: `A →{S₁→S₂} B`. Rust's type system (typestate pattern + borrow checker + must_use) already provides equivalent guarantees through different mechanisms. Adding effect annotations would fight the language rather than use it.

## Scribble / External Code Generation

EAct's implementation uses Scribble protocol descriptions → CFSM → generated Scala APIs. Pane is Rust-native. If protocol-to-code generation is ever needed, use proc macros or build scripts, not an external toolchain. But typed enums + typestate are likely sufficient.

## Dynamic Linearity Checking

EAct checks linear usage of state type objects dynamically (multiple uses caught at runtime, treated as failures). Rust's ownership system handles this statically — `Chan<S,T>` is consumed by `send()`/`recv()`. Where linearity can't be fully static, use `#[must_use]` and Option-take patterns rather than runtime linearity checks.

## `suspend` as Named Primitive

EAct's `suspend(handler, state)` is a first-class construct that installs a handler and yields to the event loop. In pane, the Handler method returning `Flow::Continue` IS the suspend — the method returns, the looper loops, state persists as `&mut self` on the Handler. No need for an explicit suspend construct.

## `become` / `ibecome` (Session Switching)

EAct's session switching freezes a send-state session and activates it later via a request queue. This matters when an actor blocks on a send in one session and needs to switch to another. Pane's active phase uses non-blocking sends (mpsc channel) — there's no send state to freeze. If cross-session causality is needed, `Messenger::send_message()` (self-delivery) already provides it: receive in session A, post a message to yourself, handle it later to send in session B.

## Access Points as Separate Abstraction (Now)

EAct's `newAP[Protocol]` + `register(ap, role, callback)` is powerful but premature for pane. Multiple protocol relationships now exist (Lifecycle, Display, Clipboard, Routing, and application-defined protocols via Protocol + Handles<P>). DeclareInterest + ServiceRouter already provide access-point semantics. Revisit the full access point model if service discovery needs grow beyond the current service map.

## Multiparty Session Types on the Wire

EAct uses multiparty session types (3+ roles). Pane's protocols are naturally binary (compositor↔client, pane↔pane, pane↔service). Multiparty types are valuable for REASONING about protocols (write a global type, project to local types, verify consistency) but the wire implementation should use binary channels. Use multiparty thinking as a design tool, not an implementation mechanism.

## General Principle

EAct's value to pane is in its DESIGN PRINCIPLES (heterogeneous session loop, sub-protocol typestate, conversation-level failure), not in its specific FORMAL CONSTRUCTS (effect system, suspend, become). Pane should absorb the insights while expressing them in Rust-native idioms.
