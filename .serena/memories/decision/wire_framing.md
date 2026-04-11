---
type: decision
status: current
supersedes: [pane/wire_framing_analysis]
sources: [pane/wire_framing_analysis]
verified_against: [docs/architecture.md@2026-04-05, crates/pane-session/src/codec.rs]
created: 2026-04-05
last_updated: 2026-04-11
importance: high
keywords: [wire_framing, ProtocolAbort, 0xFF, FrameCodec, I11, I12, service_discriminant, max_message_size, monotonic, known_services]
related: [architecture/looper, decision/server_actor_model, reference/haiku/internals]
agents: [pane-architect, formal-verifier, session-type-consultant]
---

# Wire framing safety analysis (2026-04-05)

## Two issues fixed

1. **ProtocolAbort `[0xFF][0xFF]` ambiguous** — spec didn't
   state whether it uses length-prefixed framing or raw bytes.
   **Resolution:** must go through framing (`[length=1][0xFF]`).
2. **Service discriminant 0xFF collision** — spec said 256-slot
   ceiling, meaning 0xFF was assignable. If assigned, frames
   for service 255 whose postcard payload starts with 0xFF
   would false-positive the sentinel check. **Resolution:**
   reserve 0xFF; ceiling becomes 254 non-control services
   (1..=0xFE).

(Note: when the wire format was widened to u16 in session 2,
the reserved sentinel became `0xFFFF` and the ceiling became
65534.)

## I11 restatement

Safety comes from **reserving discriminant 0xFFFF**, NOT from
postcard varint internals. The "not valid postcard" argument
is fragile (encoding-internal, version-dependent). Drop it.

## I12 two-level split

- **Framing layer:** discriminant never in any
  `InterestAccepted` → connection-fatal
- **Looper:** discriminant was accepted but revoked → discard
  silently (async subtyping)
- Framing layer maintains monotonic "ever-known" set (never
  shrinks)

## Frame codec design

`Frame` enum (`Message` | `Abort`), `FrameError` enum
(`Oversized` | `UnknownService` | `Transport`), `FrameCodec`
with `[bool; 65535]`-equivalent `known_services` bitset (now
`HashSet`). Service 0 always known. `write_abort` separate from
`write_frame`. `register_service` panics on the reserved
discriminant.

Per-service size limits not needed. Global `max_message_size`
at framing sufficient. Per-service is looper policy.

## Applied to architecture.md

I11 justification updated, reserved discriminant documented in
the framing section, framing / looper distinction added to I12.
The `Frame` codec type signature is the implementation reference.
