# Review of `docs/architecture.md`

## Summary

This is a **remarkably thorough and opinionated** architecture specification for a distributed GUI framework. It succeeds at its core goal: unifying BeOS's actor/message model with Plan 9's namespace philosophy under a coherent formalism (EAct + CLL session types). The theoretical grounding is genuine, not cargo-culted, and the "Phase 1 Structural Invariants" section is a standout — it explicitly prevents the shortcut-driven technical debt that kills multi-phase projects.

That said, the spec's confidence ("None. The spec is implementation-ready for Phase 1") is slightly ahead of its detail in a few areas. Below are findings grouped by severity, plus clarifications and retractions from follow-up analysis.

---

## Major Concerns

### 1. `#[derive(ProtocolHandler)]` on `impl` blocks is non-standard Rust
The spec shows:
```rust
#[derive(ProtocolHandler)]
#[protocol(Clipboard)]
impl Editor { ... }
```
Standard Rust derive macros attach to *types* (`struct`, `enum`, `union`), not `impl` blocks. To make this work, you'd need a **procedural attribute macro** (`#[protocol_handler]`) rather than a derive macro. This isn't just terminology — it changes the compiler plugin architecture and error reporting. The spec should either reframe this as a procedural attribute macro on `impl` blocks, or move to a derive-on-struct pattern with a delegate field.

### 2. `send_request` serialization boundary is underspecified for IPC
The spec shows:
```rust
pub fn send_request<H, R>(
    &self,
    target: &Messenger,
    msg: impl Send + 'static,
    ...
)
```
For cross-process (or cross-machine) communication, `msg` and `R` must serialize/deserialize to the wire. The in-process downcast story ("guaranteed to succeed") does not extend across the network. The spec needs either an implicit `Protocol` bound requiring `Serialize + DeserializeOwned`, or a trait-object serialization layer. Don't assume the signature works as-written for remote connections.

### 3. `Message` enum is not forward-compatible for closure handlers
The closure form `pane.run(source, |proxy, msg| match msg { ... })` exposes `Message` directly. Because `Message` is a bare `enum`, adding `Observer(ObserverNotification)` in Phase 3 will break any user code that exhaustively matches on it. Unless `#[non_exhaustive]` is added or the type is made open-ended, `Message` is not a stable public API for closure handlers.

### 4. `request_received` uses `Box<dyn Any + Send>` with no downcast convention
The spec defends `Box<dyn Any + Send>` as "intentional — the server side is fundamentally open." But it never specifies **how** the receiver identifies the payload type. Without a convention (`TypeId` matching? Magic `What` constants? A wrapper enum?), every handler will invent its own ad-hoc protocol. At minimum, the spec should define a `RequestEnvelope` with a `TypeId` or protocol UUID.

### 5. I8 (no blocking in handlers) is only *partially* enforced
The spec claims I8 is "enforced at runtime (panic in all builds)" via thread-local `CURRENT_CONNECTION`. This only catches **self-deadlock on the looper thread** — it does *not* prevent a worker thread from calling `send_and_wait` and deadlocking with the looper on the same Connection. Don't treat this as a complete safety guarantee.

---

## Medium Concerns

### 6. `FilterAction::Transform` can produce semantic nonsense
A filter can transform `KeyEvent` → `CommandExecuted`. The spec notes this is a convention, not enforced. But `CommandExecuted` carries `command: String, args: String` — a filter producing this from a `KeyEvent` would need to synthesize those strings. More importantly, a buggy filter could inject arbitrary commands. Consider whether filters should be restricted to a subset of `Message` variants, or at least require `Transform` to preserve the original variant's "domain."

### 7. Batch semantics vs. cross-Connection causality are in tension
The spec claims:
> "The unified batch linearizes events from all sources into a total order within each dispatch cycle."

But then says cross-Connection events are **not causally ordered**. A "total order" implies a deterministic sequence, but without causality, handlers still can't assume `event A` happened-before `event B` just because A preceded B in the batch. This is technically consistent (sequential consistency without causal consistency), but the terminology may mislead implementers. Consider clarifying: *processing order* ≠ *happens-before*.

### 8. `RoutingHandler` appears in resolved questions but not the main text
Resolved question #14 says `RoutingHandler: Handler`, but the main spec barely defines `RoutingHandler` (only one passing mention). If it's a first-class concept, it deserves a section. If it's deferred to Phase 3, the resolved question should reference that.

### 9. `Message` enum conflates base protocol and service notifications
The `Message` enum nests service notifications:
```rust
pub enum Message {
    Ready, CloseRequested, /* ... base protocol ... */
    Clipboard(ClipboardNotification),
}
```
This means **adding a new framework service requires modifying the top-level `Message` enum**, which is a breaking change for every crate that exhaustively matches on `Message`. The spec says "Top-level grows O(services), not O(services × events)," but O(services) breaking changes are still breaking changes. Consider making the top-level enum open-ended (e.g., `ServiceNotification { service_uuid: Uuid, payload: ... }`) so new services are additive.

### 10. Display protocol duality
Display is described as "part of the base protocol (declared in handshake, not via DeclareInterest)" and also as a type implementing `Protocol`:
```rust
struct Display;
impl Protocol for Display {
    const SERVICE_ID: ServiceId = ServiceId::new("com.pane.display");
    type Message = DisplayMessage;
}
```
If Display is never `DeclareInterest`'d, then `Handles<Display>` is never registered via the standard service path. How does the looper dispatch `DisplayMessage`? The spec says it's "bundled in `ControlMessage`," but then `DisplayMessage` is a separate type. The dispatch path from `ControlMessage::Display(...)` to `DisplayHandler` methods should be explicitly specified.

---

## Minor Issues / Nitpicks

### 11. `Geometry` uses `f64` for logical coordinates but `f32` for scale factor
HiDPI displays often have non-integer scale factors (e.g., 1.5, 2.75). `f32` is sufficient, but mixing `f64` and `f32` in the same struct is slightly awkward. Consider `f64` throughout, or document why scale factor is intentionally lower precision.

### 12. `Flow` and `Result<Flow>` overlap in meaning
`Flow::Continue` / `Flow::Stop` and `Ok(...)` / `Err(...)` create four combinations:
- `Ok(Continue)`: normal
- `Ok(Stop)`: graceful exit
- `Err(_)`: crash
- `Err` + what `Flow`? (Unused, but the type permits it)

This is fine, but the spec could be clearer about whether `Err(Stop)` is meaningful or nonsensical.

### 13. `CancelHandle` Drop being a no-op is correct
I initially noted this as a minor tension with the linear discipline, but it's well-reasoned: you don't want an early return or panic to silently cancel a request the handler actually wants to complete. This is fine — no revision needed here.

### 14. `dev-certs` tooling is hand-waved
Resolved question #25 says: "Ship `pane dev-certs` tooling. Local CA + server cert + client cert, one command." This is a product requirement, not an architectural one, and it's significantly more complex than one command on most systems (keychain integration, trust stores, SANs). I'd move this to a product/ops doc.

---

## Retractions / Clarifications

The following items were raised during initial analysis but revised upon follow-up:

- **`ServiceId` with `&'static str` is correct.** I initially flagged this as a limitation for dynamic services, but it is intentional and consistent with the "functoriality principle." Reverse-DNS service names are compile-time constants in this design. Dynamic plugins can be a future extension; do **not** refactor `ServiceId` to `String` or `Cow`.
- **`CancelHandle` Drop = no-op is the right call.** See #13 above.
- **`Message` needs `#[non_exhaustive]` or redesign** if closure handlers are to remain source-compatible across framework upgrades. See #3 above.

---

## What's Excellent

1. **Phase 1 Structural Invariants table** — This is the most valuable section. It explicitly prevents the "we'll fix it in Phase 2" trap.
2. **Linear discipline** — The invariants I1-I9 and S1-S6 are concrete and verifiable. This is rare in architecture docs.
3. **Multi-server by design** — Building distribution in from the start (even with N=1) avoids the "local-only framework that later grows remote" failure mode seen in countless GUI toolkits.
4. **Request/reply via Dispatch** — Eliminating `reply_received(token, payload)` from `Handler` is a genuine improvement over BeOS's BMessage model.
5. **Headless-first** — "Display is a capability that panes opt into, not the default" is architecturally correct and well-defended.

---

## Verdict

**The spec is ~90% implementation-ready for Phase 1.** The remaining 10% is:
- Clarifying the derive macro syntax (derive vs. attribute macro)
- Specifying how `Box<dyn Any + Send>` is identified/typed in `request_received`
- Explaining where serialization bounds enter `send_request` for IPC
- Resolving the `Display` protocol dispatch path ambiguity
- Deciding whether `Message` gets `#[non_exhaustive]` or an open-ended redesign

I recommend resolving those five items before cutting the first crate, but the overall design is sound and the formal foundations are well-integrated.
