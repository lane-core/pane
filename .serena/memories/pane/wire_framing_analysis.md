# Wire Framing Safety Analysis (2026-04-05)

## Two moderate issues

1. **ProtocolAbort `[0xFF][0xFF]` ambiguous** — spec doesn't state whether it uses length-prefixed framing or raw bytes. Must go through framing (`[length=1][0xFF]`).
2. **Service discriminant 0xFF collision** — spec says 256-slot ceiling, meaning 0xFF is assignable. If assigned, frames for service 255 whose postcard payload starts with 0xFF would false-positive the sentinel check. Fix: reserve 0xFF, ceiling becomes 254 non-control services (1..=0xFE).

## I11 restatement

Safety comes from reserving discriminant 0xFF, NOT from postcard varint internals. The "not valid postcard" argument is fragile (encoding-internal, version-dependent). Drop it.

## I12 two-level split

- **Framing layer:** discriminant never in any InterestAccepted → connection-fatal
- **Looper:** discriminant was accepted but revoked → discard silently (async subtyping)
- Framing layer maintains monotonic "ever-known" set (never shrinks)

## Frame codec design

Frame enum (Message | Abort), FrameError enum (Oversized | UnknownService | Transport), FrameCodec with `[bool; 255]` known_services bitset. Service 0 always known. write_abort separate from write_frame. register_service panics on 0xFF.

Per-service size limits not needed. Global max_message_size at framing sufficient. Per-service is looper policy.

**How to apply:** Update architecture.md I11 justification, reserve 0xFF in framing section, add framing/looper distinction to I12. Use Frame codec type signature as implementation reference.