---
name: Phase 1 pane-session pre-implementation review
description: ProtocolAbort, SessionEnum, handshake compat review -- two new invariants I10/I11, abort must use framing layer, Cow<'static, str> for ServiceId name
type: project
---

Phase 1 pane-session review completed 2026-04-03. Three changes: ProtocolAbort, SessionEnum derive, handshake compatibility.

**ProtocolAbort:** Conditionally sound. Closes the "Chan has no Drop" gap from the 2026-03-31 audit. Key finding: abort frame [0xFF, 0xFF] MUST go through the framing layer (send_raw/write_framed) not raw bytes, otherwise framing mismatch on the receiver side. Check for sentinel in Chan::recv() not Transport::recv_raw() to keep transport layer clean.

**Two new invariants:**
- I10: Chan Drop must not block. ReconnectingTransport excluded from Chan (its try_reconnect could block 60s in Drop).
- I11: [0xFF, 0xFF] is not valid postcard -- holds by varint construction (both bytes have continuation bit set, no terminator). Needs regression test.

**SessionEnum:** Sound. ChoiceOf<E>/OfferOf<E> duality correct (involution verified). Derive must enforce #[session_tag] uniqueness at compile time. Continuations in offer enum must be dualized by the macro.

**ServiceId name field:** Recommend `Cow<'static, str>`. Satisfies const construction (service_id! macro), serde Deserialize (Cow deserializes to Owned variant), and identity comparison (on UUID only via custom PartialEq/Hash). Cow::Borrowed is const as of Rust 1.83.

**Why:** Establishes soundness baseline before implementation begins.
**How to apply:** Reference when reviewing the actual implementation. Verify I10 and I11 are documented in the spec. Verify abort check is in Chan layer not Transport layer.
