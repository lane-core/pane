---
type: analysis
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [handshake, wire_format, extensibility, Tversion, postcard, version_negotiation, 9P2000, 9P2000.L, 9P2000.u]
sources: [reference/plan9/man/5/version, reference/plan9/man/5/0intro, decision/connection_source_design]
related: [decision/connection_source_design, reference/plan9/man_pages_insights, architecture/session]
agents: [plan9-systems-engineer]
---

# Handshake wire format extensibility analysis

Lane asked for Plan 9 version negotiation deep dive, 9P wire
format evolution analysis, and recommendation for pane's Hello/
Welcome extensibility given postcard's positional format.

## 1. 9P Tversion/Rversion exact mechanics

Wire format (version(5)):

    size[4] Tversion tag[2] msize[4] version[s]
    size[4] Rversion tag[2] msize[4] version[s]

Exactly two fields negotiated:
- `msize` (u32): max message size. Client proposes, server
  may reduce but never increase. Both honour it thereafter.
- `version` (string, length-prefixed): protocol variant.

Tag must be NOTAG (0xFFFF). Tversion must be the first message.
No further requests until Rversion received.

Critical: a successful Tversion **resets the connection**. All
outstanding I/O aborted, all fids freed. This is why it must be
first — it's the session constructor, not just a handshake.

If the server doesn't understand the client's version string,
it replies with Rversion + version="unknown" (NOT Rerror). This
is important: version mismatch is not an error, it's a clean
rejection that the client can parse at the Rversion level.

## 2. Version string as sole gate mechanism

9P2000 had one version: "9P2000". That's it. The string
selected the entire field layout for all subsequent messages.

**9P2000.u** (Unix extension by Plan 9 from User Space / v9fs):
Changed the Tversion string to "9P2000.u". Same Tversion wire
format — msize[4] + version[s]. The version string gates:
- Tattach gains n_uname[4]
- Tcreate gains extension[s]
- Tstat/Twstat gain uid/gid/muid extension strings + n_uid etc.
- Rerror gains errno[4]

No capability bits. No feature flags. The version string is the
sole discriminant. If the server responds with "9P2000" instead
of "9P2000.u", the client knows to fall back to the base
protocol.

**9P2000.L** (Linux VFS extension for QEMU/virtio-9p):
Same Tversion wire format. String "9P2000.L". Gates a complete
replacement message set: Tlcreate, Tsymlink, Tmknod, Trename,
Treadlink, Tgetattr, Tsetattr, Txattrwalk, Txattrcreate,
Treaddir, Tfsync, Tlock, Tgetlock, Tlink, Tmkdir, Trenameat,
Tunlinkat. These messages have new type byte values. The
type byte in each message identifies which message it is; the
version string determines which type bytes are legal.

Again: no capability bits, no feature negotiation, no extension
map. Purely "if version string is X, these messages exist."

**The period convention** (version(5) lines 70-78): Version
strings use period-separated suffixes. "9P2000.u" has base
"9P2000" and suffix "u". The server may respond with a lower
version by numeric comparison of the base digits. This allows
forward-compatible negotiation: a client sending "9P2001" to
a 9P2000 server gets "9P2000" back.

## 3. Why 9P never needed extension maps

Three reasons the version-string-only approach worked for 9P:

1. **Rare evolution.** 9P changed twice in 15 years (2000 →
   2000.u → 2000.L). Handshake extensibility is only needed if
   the handshake changes often. 9P's didn't.

2. **Monolithic version jumps.** Each version was a coherent
   bundle of changes. There was no "I support getattr but not
   xattr" — you spoke 9P2000.L or you didn't. The version
   string selected a complete dialect.

3. **No optional features within a version.** If you negotiate
   9P2000.L, you must support all of 9P2000.L. No partial
   implementations allowed. This kept the combinatorial space
   at 1.

## 4. Mixed text/binary format: deliberate?

9P's Tversion has a binary msize (u32 LE) and a text-ish
version string (length-prefixed UTF-8). But this is not
"self-describing" — it's the same binary framing as every
other 9P message. The version string is a string type field
within a fixed binary layout. There's no JSON, no TLV, no
key-value pairs. The "text" is just a string-typed field
with known position.

9P's philosophy was: text for human interaction
(ctl files, proc files, factotum RPC), binary for wire
efficiency. The version string is text because it's a
protocol name humans need to read in diagnostics and logs.
The rest of the handshake is binary because it's machine-parsed.

## 5. Analysis of pane's three options

### Option A — Version gates schema (9P's approach)

version: u32 selects the Hello/Welcome field layout.

**Pro:** Simple, proven, zero overhead. Compiler switch on version
selects the right struct. Adding max_outstanding_requests means
version 2. Version 1 clients get version 1 layout back.

**Con:** Every new field means a new version number and a new
struct. If pane's handshake evolves faster than 9P's did (likely),
this creates a pile of HandshakeV1/V2/V3/... structs. But 9P
also shows that handshakes DON'T evolve fast if the design is
right — 9P needed 3 versions in 25 years.

**The real 9P lesson:** The version-first approach worked because
9P versioned the whole protocol, not just the handshake. The
version string selected the complete message vocabulary. pane's
version field currently just gates handshake layout, which is
weaker. If pane ever needs to add new ControlMessage variants,
the version should gate those too — otherwise version is only
half the story.

### Option B — Self-describing handshake, binary data plane

JSON/CBOR for Hello/Welcome. Postcard for data frames.

**Pro:** Future-proof. Unknown fields ignored. Handshake is once
per connection so the cost is negligible (microseconds).

**Con:** Adds a second serialization format. JSON has no u16 type
(everything is f64/i64). CBOR is better but still a dependency.
Increases attack surface on the one message that runs before
authentication is validated. More code = more bugs in the code
path that must be most robust.

**9P precedent says no.** 9P used the same binary framing for
Tversion as for everything else. The simplicity was the point.
Having two serializers for a handshake that changes once every
5 years is solving a problem that doesn't exist yet.

### Option C — Extension map (postcard + HashMap<String, Vec<u8>>)

**Pro:** Backwards compatible without version bump. New fields are
optional extensions. Discoverable.

**Con:** HashMap ordering is nondeterministic. String keys add
allocation and comparison cost. The "known" extensions need to be
documented somewhere — you've just reinvented a schema but worse.
Postcard's HashMap encoding puts the key-value pairs at the end,
which works for append-only extension, but you lose type safety.

**This is the HTTP header model.** It works when you have
hundreds of independent extensions from different vendors. pane
has one vendor. The complexity is not justified.

## 6. Recommendation for pane

**Option A (version gates), enhanced with one postcard trick.**

The specific problem is: adding max_outstanding_requests to
Hello/Welcome is a breaking wire change with postcard because
postcard is positional. But postcard's `#[serde(default)]`
annotation on the last fields already handles this for
appending: if the bytes run out before all fields are decoded,
defaulted fields get their default values.

This is EXACTLY how D9 was shipped — max_outstanding_requests
has `#[serde(default)]` and defaults to 0. An old client that
doesn't send it gets 0 (unlimited). This works today.

**The constraint is: new fields can only be appended, and must
have `#[serde(default)]`.** This is the same constraint as
protobuf's "add fields at the end, make them optional." It
works for the common case.

**When postcard append stops working:** when you need to remove
a field, reorder fields, or change a field's type. At that
point, bump the version number. The version field is already
the first field in Hello — the deserializer reads version,
then selects the struct.

The hybrid recommendation:

1. **Keep postcard for Hello/Welcome.** Don't introduce a
   second serializer.

2. **Keep version as the first field.** It's already there.
   It already works.

3. **New optional parameters: append with `#[serde(default)]`.**
   This is what D9 did. It's not a hack — it's the standard
   postcard extension pattern.

4. **Breaking changes: bump version.** When the append-only
   discipline is insufficient, version 2 selects a new layout.
   The server reads version first (it's always the first u32),
   then deserializes the rest according to the version.

5. **Do NOT separate version from Hello.** 9P's Tversion was a
   separate message from Tattach, yes, but 9P had a reason:
   Tversion reset the connection and could be re-sent mid-session.
   pane's handshake runs exactly once per connection (D2). There
   is no mid-session re-negotiation. Splitting version into its
   own message adds a round-trip for no gain.

**Confidence:** High. This is exactly what 9P did, adjusted for
postcard's encoding model. The append-with-default pattern has
been proven by protobuf, Cap'n Proto, and FlatBuffers for decades.
The version bump escape hatch covers the rare breaking change.

**What you gain:** Zero new dependencies, zero new serialization
formats, zero new message types. One rule: "append, default,
bump when you can't."

**What you pay:** Field ordering in Hello/Welcome is load-bearing.
Fields cannot be reordered. Removed fields must keep their slot
(set to default). This is the protobuf discipline.

**The simpler alternative:** Do nothing. The current code with
`#[serde(default)]` already handles D9. The "extensibility
problem" is real only if you expect to add many fields often.
9P's track record says you won't.
