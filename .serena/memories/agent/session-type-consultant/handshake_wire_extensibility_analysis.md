---
type: analysis
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [handshake, wire_extensibility, postcard, cbor, session_subtyping, forward_compatibility, Hello, Welcome, Option_B]
sources: [decision/connection_source_design, decision/wire_framing, architecture/session]
verified_against: [crates/pane-session/src/handshake.rs@2026-04-11, crates/pane-session/src/bridge.rs@2026-04-11]
related: [reference/papers/eact, reference/papers/dlfactris, analysis/session_types/_hub]
agents: [session-type-consultant]
---

# Handshake wire extensibility analysis

## Verdict: Option B (self-describing handshake, binary data plane)

Three options evaluated for Hello/Welcome forward compatibility.
All three preserve the session type `Send<Hello, Recv<Result<Welcome, Rejection>>>` — encoding format is below the session-type abstraction. The distinction is at the value level.

### Key findings

1. **Session type unchanged under all options.** Payload
   encoding is invisible to session-type discipline. [FH]
   Theorem 4 (Preservation) is agnostic to byte layout.

2. **Option B restores session-subtyping.** Adding fields
   with `#[serde(default)]` in a self-describing format is
   genuine width subtyping ([Gay & Hole 2005] session
   subtyping, covariant on Send payload). Postcard destroys
   this property via positional encoding.

3. **Format boundary aligns with session boundary.** The
   handshake and data-plane are separate sequential sessions.
   Switching encoding between them is sequential composition
   of sessions with different value-level encodings — [FH]
   Theorem 4 covers this.

4. **`#[serde(default)]` on postcard is dead code.** Postcard
   uses positional deserialization (`deserialize_seq`), never
   calling the serde `default` mechanism. The annotations on
   `max_outstanding_requests` currently do nothing.

5. **No affine gap change.** No obligation handles exist
   during the handshake. Bridge thread panic = session abort,
   already covered by [FH] Theorems 6-8 (Maty_zap).

6. **No deadlock change.** Handshake is synchronous
   two-message exchange on dedicated bridge thread. No
   connectivity graph exists yet.

### Option A rejection rationale

Version-gated schemas are sound but scale poorly. Postcard
positional encoding means version-specific deserializer types
(`HelloV1`, `HelloV2`, ...). Manual dispatch on version field.
9P's Tversion used a string (self-describing) for the version
negotiation point.

### Option C rejection rationale

Extension map (`HashMap<String, Vec<u8>>`) is manual width
subtyping with three traps: must-be-last-field policy (not
type-enforced), double encoding (extension values need their
own format), no structural subtyping for core fields.

### Implementation note

Recommended format: CBOR (RFC 8949) via `ciborium`. Binary,
deterministic canonical form, mature serde support. Only
handshake frame payloads (service 0, two frames) change
encoding. Framing layer (length-prefix + service discriminant)
stays unchanged. Data-plane postcard encoding unchanged.

## Provenance

Lane surfaced the question during ConnectionSource C1
implementation after noticing `#[serde(default)]` is dead
code with postcard. Analysis grounded in [FH] EAct
preservation theorems, [Gay & Hole 2005] session subtyping,
and [JHK24] connectivity-graph analysis (confirming no
deadlock relevance during handshake).
