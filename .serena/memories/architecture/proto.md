---
type: architecture
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [pane-proto, message, protocol, handles, handler, lifecycle, control_message, service_frame, obligation, reply_port, address, peer_auth, monadic_lens, non_exhaustive, service_id]
related: [architecture/session, architecture/app, decision/wire_framing, decision/messenger_addressing, policy/non_exhaustive_extensions, policy/ghost_state_discipline, reference/papers/eact]
agents: [pane-architect, session-type-consultant, be-systems-engineer]
---

# pane-proto Architecture

## Summary

pane-proto is the protocol-vocabulary crate. Zero IO, zero
async, zero runtime: only type contracts, value types, and the
traits that downstream crates implement. 99 tests exercise
codec round-trips, construction edges, and the invariants the
types are structured to enforce. Every other pane crate
depends on this one and takes from it the universal `Message`
trait, the `Protocol` / `Handles<P>` dispatch surface, the
`Handler` lifecycle trait, the wire envelopes
(`ControlMessage`, `ServiceFrame`), the obligation handles
(`ReplyPort`, `CompletionReplyPort`, `CancelHandle`), and the
`MonadicLens<S,A>` mixed optic with its law-verified test
harness.

## Components

### Modules

- **message.rs** — `Message` marker trait. No methods; the
  bounds (`Clone + Serialize + DeserializeOwned + Send +
  'static`) *are* the contract. Blanket-implemented on
  anything satisfying them; obligation handles deliberately
  fail the bound (`!Clone`, `!Serialize`).
- **protocol.rs** — `ServiceId` (UUIDv5 deterministic derivation
  from a name, plus an XOR-fold `tag()` byte for type-erasure
  defense-in-depth), `Protocol` supertrait (`service_id()` +
  `Message` associated type), `RequestProtocol` (adds `Reply`
  type for request/reply dispatch).
- **handles.rs** — `Handles<P: Protocol>` trait, a single
  `receive(&mut self, msg: P::Message) -> Flow` method. The
  framework-agnostic dispatch interface.
- **handler.rs** — `Handler` lifecycle trait with named methods
  (`ready`, `close_requested`, `disconnected`, `pulse`,
  `pane_exited`, `quit_requested`). Blanket `Handles<Lifecycle>`
  impl converts `Handler` via exhaustive match on
  `LifecycleMessage`.
- **protocols/lifecycle.rs** — `Lifecycle` protocol +
  `LifecycleMessage` enum. Universal service 0 messaging; every
  pane speaks it without DeclareInterest.
- **address.rs** — `Address` (`pane_id: u64`, `server_id: u64`),
  Copy + Hash + Serialize, `is_local()` accessor.
  `#[non_exhaustive]`.
- **control.rs** — `ControlMessage` wire envelope for service 0,
  variants Lifecycle, DeclareInterest, InterestAccepted,
  InterestDeclined, ServiceTeardown, RevokeInterest, Cancel,
  Watch, Unwatch, PaneExited. `DeclineReason`
  (VersionMismatch, ServiceUnknown, SelfProvide,
  SessionExhausted) and `TeardownReason` (ServiceRevoked,
  ConnectionLost). All `#[non_exhaustive]`.
- **service_frame.rs** — `ServiceFrame` enum for services
  `1..=254`: Request(token, payload), Reply(token, payload),
  Failed(token), Notification(payload). Postcard-encoded
  byte payload — type safety is at the edges (sender
  serializes `P::Message`, receiver deserializes).
  `#[non_exhaustive]` for Phase 3 streaming.
- **obligation.rs** — Three move-only `#[must_use]` obligation
  types: `ReplyPort<T>` (`.reply()` or drop → `ReplyFailed`),
  `CompletionReplyPort` (`.complete()` or drop →
  `CompletionFailed`), `CancelHandle` (Drop is a no-op,
  `.cancel()` is an active abort — inverted polarity by
  design). Closure-erased backends (`Box<dyn FnOnce>`) let
  the same types carry in-process, wire, or stub behavior.
- **peer_auth.rs** — `PeerAuth` (`uid`, `AuthSource`).
  `AuthSource` enum: `Kernel { pid }` or `Certificate {
  subject, issuer }`. Transport-derived, not wire-transmitted.
  Eq/Hash are over the full struct, so same uid via different
  sources compare as distinct.
- **filter.rs** — `MessageFilter<M>` trait +
  `FilterAction<M>` (Pass, Transform(M), Consume). Runs in
  registration order; obligation handles bypass filtering.
- **flow.rs** — `Flow` enum returned from every handler
  method: Continue or Stop.
- **exit_reason.rs** — `ExitReason` enum wire-transmitted in
  PaneExited: Graceful, Disconnected, Failed, InfraError.
- **monadic_lens.rs** — `MonadicLens<S,A>`, a mixed optic with
  pure view (`fn(&S) -> A`) and effectful set (`fn(&mut S,
  A) -> Vec<Effect>`), `Effect` being Notify or SetContent.
  `AttrReader<S>`, `AttrWriter<S>`, `AttrSet<S>` derived for
  type-erased read/write access. `assert_monadic_lens_laws`
  harness verifies GetPut, PutGet, PutPut on concrete lenses.

### Public API surface

`lib.rs` re-exports the main traits and types:
`Message`, `Protocol`, `RequestProtocol`, `ServiceId`,
`Handles<P>`, `Handler`, `Flow`, `ExitReason`, `Address`,
`ControlMessage`, `DeclineReason`, `TeardownReason`,
`PeerAuth`, `AuthSource`, `ServiceFrame`, `MessageFilter<M>`,
`FilterAction<M>`. The submodules are all `pub`. No
proc-macros live here; derive macros (if any) are in
downstream crates.

### Notable patterns

- **Blanket `Handles<Lifecycle>` over `Handler`.** No
  separate lifecycle dispatch path — everything goes
  through `Handles<P>` fn-pointer dispatch, and `Handler` is
  just named-method ergonomics.
- **Type-erased dispatch with edge-typed safety.**
  `ServiceFrame` holds `Vec<u8>`; (de)serialization happens
  where the concrete `P::Message` type is known. Avoids
  generic bloat while keeping the type contract at the
  application boundary. `ControlMessage` is the one exception
  (directly postcard-encoded, not wrapped in `ServiceFrame`).
- **Closure-erased obligations.** `ReplyPort<T>` wraps `Box<dyn
  FnOnce>` rather than a concrete channel type, so the same
  obligation handle works across in-process, wire, and
  stub backends. Backend is a construction-time decision,
  invisible to the handler.
- **`ServiceId` tag byte as type-erasure defense.** XOR-fold
  of the UUID gives a 1-byte tag with ~255/256 collision
  resistance across independent UUIDs. Caught at the FrameCodec
  layer to detect routing bugs at zero coordination cost.
- **`MonadicLens` as Be-attribute heir.** Pure view
  (comonadic) + effectful set (Kleisli over a writer monad).
  Same lens serves reads, writes, and FUSE attribute names.

## Invariants

Structurally enforced (the type system rejects violations at
the call site):

- **I5 (Clone-safe `Message`)** — the `Message` trait requires
  `Clone`, enabling filtering, logging, projection without
  ownership transfer. Obligation handles deliberately fail
  the `Clone` bound to keep reply-port semantics linear.
- **Obligation linearity** — `ReplyPort<T>`,
  `CompletionReplyPort` are move-only and `#[must_use]`.
  `.reply()` / `.complete()` consume `self`; forgetting fires
  the failure compensation on Drop. Panic-unwind tests
  verify the Drop path during unwinding (`obligation.rs:350`).
- **`CancelHandle` inverted polarity** — Drop is a no-op,
  `.cancel()` is an active abort. Opposite polarity to the
  other obligation types, so the "optional abort" intent is
  explicit at the API surface.
- **`ServiceId` determinism** — `ServiceId::new(name)` always
  produces the same UUIDv5. The tag byte derives from the
  UUID, not from the name. Wire-stable by construction.
- **`PeerAuth` full comparison** — Eq and Hash cover the full
  struct so `uid=1000` authenticated via kernel peer creds is
  not equal to `uid=1000` via certificate. The distinction is
  preserved through any HashMap keyed on `PeerAuth`.
- **`#[non_exhaustive]` on every evolving type** —
  `ControlMessage`, `DeclineReason`, `TeardownReason`,
  `ServiceFrame`, `Address`, `PeerAuth`, `AuthSource`. Per
  `policy/non_exhaustive_extensions`, this keeps Phase 3 wire
  additions from breaking downstream exhaustive matches.
- **`Handler` exhaustiveness on `LifecycleMessage`** — the
  blanket `Handles<Lifecycle>` impl uses an exhaustive match,
  so adding a lifecycle variant fails compilation until
  `handler.rs` is updated.
- **Session id capacity (I10/I11/I12 adjacent)** — `session_id`
  is `u16`; `DeclineReason::SessionExhausted` documents the
  65534 per-connection limit. The framing invariants
  themselves (ProtocolAbort = 0xFFFF, unknown discriminant
  handling) are enforced by pane-session's FrameCodec — this
  crate only declares the discriminant vocabulary.

Runtime checked:

- **`MonadicLens` laws** — `assert_monadic_lens_laws` exercises
  GetPut / PutGet / PutPut on concrete lenses
  (`monadic_lens.rs:255`). Violations are test failures, not
  compile errors.

## Status and gaps

- Phase 2 vocabulary is complete. No TODO / FIXME markers in
  source.
- "Eager Hello.interests" is intentionally **not** modelled in
  this crate; service negotiation is looper-managed. pane-proto
  exposes the `DeclareInterest` / `InterestAccepted` vocabulary
  and leaves the active-phase choice to pane-app.
- The `HandlesRequest<P>` trait lives in pane-app as a
  convenience layer over `ReplyPort` callbacks; pane-proto
  stops at the `Handles<P>` / obligation surface.
- pane-fs's `AttrReader` / `AttrSet` are *deliberately
  separate* from pane-proto's `MonadicLens` layer — pane-fs
  wants a snapshot-only FUSE read path, not the effectful
  writer monad (`architecture/fs`).

## See also

- `architecture/session` — framing, codec, transport, server,
  where the wire invariants from `ControlMessage` /
  `ServiceFrame` actually land.
- `architecture/app` — `HandlesRequest<P>`, dispatch,
  ServiceHandle, Messenger, and the obligation backends that
  wire `ReplyPort` to real request/reply flows.
- `decision/wire_framing` — ProtocolAbort framing, reserved
  discriminant, I11/I12 split.
- `decision/messenger_addressing` — why Address is a value
  type and how it composes with Messenger / ServiceHandle.
- `policy/non_exhaustive_extensions` — audit obligations for
  the `#[non_exhaustive]` wire types.
- `policy/ghost_state_discipline` — typestate over
  correlation IDs, informs the obligation handle design.
- `reference/papers/eact` — Fowler-Hu calculus that provides
  the E-Suspend / E-React / single-mailbox grounding the
  `Handles<P>` dispatch plus obligation semantics rest on.

**Files:** `crates/pane-proto/src/{message,protocol,handles,handler,address,control,service_frame,obligation,peer_auth,filter,flow,exit_reason,monadic_lens}.rs`, `crates/pane-proto/src/protocols/lifecycle.rs`. Tests in `crates/pane-proto/tests/` and per-module `mod tests`.
