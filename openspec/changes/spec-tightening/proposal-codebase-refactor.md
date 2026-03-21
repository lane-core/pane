# Codebase Refactor Proposal: Aligning with Architecture Spec

Assessment of the current Rust codebase against the architecture spec (2026-03-21). What exists, what stays, what goes, what's missing, and in what order.

---

## 1. Current State

Two crates exist:

### pane-proto (crates/pane-proto)

**What it does:** Defines wire types for the pane protocol and a runtime state machine for protocol validation.

| File | Contents | Lines | Verdict |
|---|---|---|---|
| `wire.rs` | postcard serialize/deserialize + length-prefixed framing | 33 | **Keep** |
| `message.rs` | `PaneId`, `PaneKind`, `PaneRequest`, `PaneEvent`, `RouteMessage` | 103 | **Restructure** |
| `event.rs` | `KeyEvent`, `MouseEvent`, `Modifiers`, `Key`, etc. | 121 | **Keep** |
| `tag.rs` | `TagLine`, `TagAction`, `TagCommand`, `BuiltInAction` | 55 | **Keep** |
| `cell.rs` | `Cell`, `CellAttrs`, `CellRegion` | 107 | **Move** (becomes pane-comp internal or pane-ui) |
| `color.rs` | `Color`, `NamedColor` (xterm-256 palette) | 34 | **Keep** |
| `attrs.rs` | `AttrValue` (dynamic BMessage-style attr bag), `PaneMessage<T>` wrapper | 123 | **Rethink** |
| `polarity.rs` | `Value` and `Compute` marker traits | 10 | **Delete** |
| `state.rs` | `ProtocolState` runtime state machine | 167 | **Delete** |
| `widget.rs` | `WidgetNode`, `WidgetEvent` | 73 | **Remove from proto** (Phase 7 concern, belongs in pane-ui) |
| `server/mod.rs` | `ServerVerb` (Query/Notify/Command) | 23 | **Delete** |
| `server/views.rs` | `TypedView` trait, `Set`/`Unset` typestate, helpers | 94 | **Delete** (replaced by session types) |
| `server/roster.rs` | `RosterRegister`, `RosterServiceRegister` + builders | 199 | **Delete** (redesign as session type protocol) |
| `server/route.rs` | `RouteCommand`, `RouteQuery` + builders | 167 | **Delete** (router eliminated) |
| `tests/roundtrip.rs` | proptest roundtrip for all wire types + state machine tests | 451 | **Rewrite** (keep proptest approach, new types) |
| `tests/typed_views.rs` | Tests for TypedView parsing | 187 | **Delete** |

**Dependency audit:**
- `serde` + `postcard` — keep, these are the wire format
- `bitflags` — keep, used well for `CellAttrs` and `Modifiers`
- `optics = "0.3"` — **delete**. Listed in Cargo.toml, imported nowhere. Unused dependency from an earlier design phase.
- `proptest` (dev) — keep

### pane-comp (crates/pane-comp)

**What it does:** Renders a single hardcoded pane in a winit window using smithay's GLES renderer. No Wayland protocol handling, no client connections, no input dispatch. It's a proof-of-concept that smithay + cosmic-text can render pane chrome (tag line, beveled borders, body background, glyph atlas).

| File | Contents | Lines | Verdict |
|---|---|---|---|
| `main.rs` | winit backend init, render loop with `thread::sleep(16ms)` | 170 | **Rewrite** (Phase 4) |
| `pane_renderer.rs` | Hardcoded pane rendering, color conversion | 252 | **Rewrite** (Phase 4) |
| `glyph_atlas.rs` | CPU-side glyph rasterization via cosmic-text | 284 | **Keep and develop** (Phase 4) |

**What's demonstrated vs what's needed:**

The first pane render (2026-03-16) proved smithay's GLES renderer works for solid rects and that cosmic-text can build a glyph atlas. Good. But the compositor currently has:
- No calloop event loop (uses `thread::sleep` polling)
- No Wayland protocol handling (pure winit demo)
- No client socket (no pane protocol server)
- No input dispatch
- No layout tree
- No per-pane threads

Everything except the glyph atlas concept needs to be rebuilt from scratch for Phase 4. The glyph atlas approach is sound and the code is reasonable — it needs GPU upload (currently CPU-only data) and integration with a texture-based rendering pipeline, but the rasterization logic is reusable.

---

## 2. What's Stale — The Old Architecture Artifacts

These are constructs from the pre-spec architecture that the spec explicitly supersedes:

### `ProtocolState` runtime state machine (state.rs)

This is a runtime enum (`Disconnected | Active { panes, pending_creates }`) that validates protocol transitions by cloning and mutating HashMap state. The architecture spec replaces this with compile-time session types — the `Chan<S, Transport>` typestate pattern where invalid transitions don't compile. The runtime state machine is the thing session types make unnecessary.

**Why it can't be adapted:** The whole point of session types is that `ProtocolState::apply()` disappears. Instead of "apply request to state, get back new state or error," the type IS the state: `Chan<Recv<PaneRequest>, ...>` only offers `recv()`, which produces the next state. There's nothing to migrate — it's a different paradigm.

### `Value` / `Compute` polarity traits (polarity.rs)

Marker traits with no methods. `impl Value for PaneRequest {}`, `impl Value for TagLine {}`, etc. These were conceptual scaffolding from a sequent calculus framing that the foundations spec dropped ("drop sequent calculus framing" — commit 40ba677). No code depends on these traits for dispatch, bounds, or type-level reasoning. They're documentation masquerading as code.

### `PaneMessage<T>` wrapper (attrs.rs)

This wraps a typed core with a `Vec<(String, AttrValue)>` attrs bag, modeling BMessage's pattern of "typed core + extensible attrs." The architecture spec takes a different path: messages are typed Rust enums serialized with postcard. The spec says: "stronger typing (compile-time field access) with the same loose coupling."

The attrs bag is BMessage's weakest feature translated faithfully. BMessage needed it because C++ didn't have sum types. Rust does. The session-typed protocol handles what the attrs bag was trying to do — carrying additional context — by having variants that include the data directly in the enum. If you need extensibility, you add a variant to the enum and every consumer that doesn't handle it gets a compiler error. That's better than silent key-value bags.

`AttrValue` itself (the dynamic type: String/Int/Float/Bool/Bytes/Attrs) may still be useful for the filesystem interface or the store's attribute values, but it doesn't belong in the protocol layer. Move it to pane-store-client when that crate exists.

### `ServerVerb` + `TypedView` + route/roster modules (server/)

The `ServerVerb` enum (Query/Notify/Command) with `PaneMessage<ServerVerb>` and typed view parsing was the inter-server protocol. The architecture spec eliminates both the concept and the infrastructure:

- The central router is gone. `RouteCommand`, `RouteQuery`, and the entire `server/route.rs` module have no home.
- Inter-server communication is now session-typed conversations, not verb+attrs messages. The `TypedView` pattern (parse a bag of attrs into a typed struct at runtime) is exactly what session types replace with compile-time verification.
- The `Set`/`Unset` typestate builder pattern for constructing messages is clever, but it's solving the wrong problem. With session types, message construction is just constructing an enum variant — no builder needed because the type system already enforces required fields.

### `RouteMessage` (message.rs)

Carried routing context (src, dst, wdir, content_type, attrs, data) through a central router. The router is eliminated; routing is kit-level. This type needs to be redesigned as part of the pane-app kit's routing protocol, not as a wire type in pane-proto.

### `PaneKind::CellGrid` — the compositor-renders-cells model

`PaneKind` has three variants: `CellGrid`, `Widget`, `Surface`. The architecture spec is clear: **client-side rendering**. The compositor composites client buffers; it doesn't render body content. `CellGrid` (compositor renders text from cell data) and `Widget` (compositor renders widget tree) are both server-rendered models. Only `Surface` (client renders pixels) aligns with the spec.

This is a fundamental shift. The current `PaneRequest::WriteCells` and `PaneRequest::SetWidgetTree` messages have the compositor doing the rendering work. The spec says the pane-ui kit handles that on the client side. The client produces a buffer (shared memory or DMA-BUF), attaches it via wl_surface, and the compositor composites it.

The `Cell`, `CellAttrs`, `CellRegion` types are still useful — they move to the pane-ui kit's text rendering layer, where the client uses them to render into its own buffer. But they leave pane-proto.

### `WidgetNode` / `WidgetEvent` (widget.rs)

Same issue: server-rendered widget tree. In the new architecture, widget rendering is the pane-ui kit's job (client-side, Phase 7). This is way premature for pane-proto. Remove entirely.

---

## 3. What's Salvageable

### Wire framing (wire.rs) — keep as-is

Length-prefixed postcard serialization. Correct, minimal, well-tested. This is the foundation of the transport layer.

### Input types (event.rs) — keep as-is

`KeyEvent`, `MouseEvent`, `Modifiers`, `Key`, `NamedKey`, `FKey` with validated construction. These are well-designed wire types that the compositor will send to clients. They survive the architectural transition unchanged.

### Tag types (tag.rs) — keep, evolve

`TagLine`, `TagAction`, `TagCommand`, `BuiltInAction` model the tag line concept that the spec commits to. The structure (name + built-in actions + user actions) is correct. These stay in pane-proto because the tag content travels through the pane protocol — client sends tag content, compositor renders it.

One refinement: the spec says "text IS the interface" — the tag line is editable text, not a structured list of actions. The current `Vec<TagAction>` model may be too structured. The tag line might just be a string with executable regions. But this is a design decision for Phase 4, not a structural problem with the type.

### Color types (color.rs) — keep

`Color` and `NamedColor` with the xterm-256 palette. Needed everywhere.

### PaneId (message.rs) — keep

Compositor-assigned opaque identifier. Correct and minimal.

### Glyph atlas approach (glyph_atlas.rs) — keep concept, evolve code

The approach is right: rasterize with cosmic-text, cache in an atlas, use UV coordinates for rendering. The code needs:
- GPU texture upload (currently CPU-only)
- Integration with the compositor's rendering pipeline
- Shared memory atlas for pane-ui kit (cross-process glyph sharing)

But the core logic — rasterize, pack into atlas, track UVs — survives.

### proptest roundtrip testing — keep pattern, rewrite tests

The property-based testing approach for wire type serialization roundtrips is exactly right. New types get new strategies, but the pattern stays.

---

## 4. What's Missing

### pane-session (new crate) — Phase 2, critical path

The custom session type implementation. This is the make-or-break Phase 2 deliverable.

**Contents:**
- `Chan<S, Transport>` — the session-typed channel
- Five primitives: `Send<T, S>`, `Recv<T, S>`, `Choose<Choices>`, `Offer<Choices>`, `End`
- `Dual<S>` — automatic duality derivation (proc macro or trait)
- `SessionError` — crash/disconnect as error, not panic
- `UnixSocketTransport` — postcard over unix domain sockets
- `CalloopSource` — `calloop::EventSource` impl for compositor-side channels
- `ThreadedEndpoint` — blocking API for client-side channels (std::thread, no async)

**Does not contain:** Message type definitions (those stay in pane-proto). Protocol definitions (those use the primitives from pane-session with the types from pane-proto).

**Acceptance criterion from the session type assessment:** pane-comp calloop main thread talks to a test client over unix socket, session-typed, with crash recovery by killing client mid-session.

### pane-proto rewrite — Phase 1, immediate

After removing the stale code, pane-proto becomes much smaller and more focused:

**Keeps:**
- `wire.rs` (framing)
- `event.rs` (input types)
- `tag.rs` (tag line types)
- `color.rs` (color palette)
- `PaneId` (from message.rs)

**Adds:**
- Session type protocol definitions using pane-session primitives
- Pane lifecycle protocol: `Connect -> CreatePane -> (Send events / Recv content)* -> Close`
- Heartbeat types: `Heartbeat(u64)` / `HeartbeatAck(u64)`
- Inter-server protocol types for roster registration, service registry queries

**The key difference:** pane-proto defines the message enum variants AND the session type that constrains their ordering. The session type is defined here; the session type machinery lives in pane-session.

### pane-notify (new crate) — Phase 3

fanotify/inotify abstraction. The spec calls this out as Phase 3 item 4, needed before pane-app kit for routing rule file watching. Linux-only, but pane is Linux-only anyway.

### pane-app (new crate) — Phase 3

Application lifecycle kit. The BLooper equivalent: thread + channel + handler chain + routing. This is where `RouteMessage` eventually lives (redesigned), along with connection management, service discovery via pane-roster, and the looper abstraction.

---

## 5. Proposed Crate Structure

### After Phase 1-2 restructure:

```
crates/
  pane-proto/        # Wire types, session protocol definitions
    src/
      lib.rs
      wire.rs        # (existing) postcard framing
      event.rs       # (existing) input types
      tag.rs         # (existing) tag line types
      color.rs       # (existing) color palette
      id.rs          # PaneId (extracted from message.rs)
      protocol.rs    # Session type protocol defs using pane-session
      heartbeat.rs   # Heartbeat/HeartbeatAck types
    tests/
      roundtrip.rs   # (rewritten) proptest for new types

  pane-session/      # Session type machinery (NEW)
    src/
      lib.rs
      types.rs       # Chan<S, Transport>, Send, Recv, Choose, Offer, End
      dual.rs        # Duality derivation
      error.rs       # SessionError
      transport/
        mod.rs       # Transport trait
        unix.rs      # UnixSocketTransport (postcard over UDS)
        memory.rs    # InMemoryTransport (for testing)
      calloop.rs     # CalloopSource impl
      thread.rs      # ThreadedEndpoint (blocking API)
    tests/
      session_smoke.rs
      transport_roundtrip.rs
      crash_recovery.rs

  pane-comp/         # Compositor (rewritten in Phase 4)
    src/
      main.rs
      glyph_atlas.rs # (evolved from existing)
      ...
```

### After Phase 3:

```
crates/
  pane-proto/
  pane-session/
  pane-notify/       # fanotify/inotify (NEW)
  pane-app/          # Application kit: looper, handler, routing (NEW)
  pane-comp/
```

### Full target (Phase 4+):

```
crates/
  pane-proto/        # Wire types, protocol defs
  pane-session/      # Session type machinery
  pane-notify/       # Filesystem notification
  pane-app/          # Application lifecycle kit
  pane-comp/         # Compositor
  pane-shell/        # Terminal pane (Phase 4)
  pane-ui/           # Interface kit: text, widgets, rendering (Phase 4+)
  pane-text/         # Text buffers, structural regexps (Phase 7)
  pane-input/        # Keybinding grammar (Phase 5)
  pane-store/        # Attribute store server (Phase 6)
  pane-store-client/ # Store client library (Phase 6)
  pane-roster/       # Application directory server (Phase 6)
  pane-watchdog/     # Health monitor (Phase 6)
  pane-fs/           # FUSE filesystem (Phase 7)
```

### Workspace Cargo.toml after Phase 2:

```toml
[workspace]
resolver = "2"
members = [
    "crates/pane-proto",
    "crates/pane-session",
]

# pane-comp requires Linux (wayland, libinput)
# Not in workspace members until Phase 4 rebuild
```

---

## 6. Concrete Deletion/Rewrite Plan

### Phase 1: pane-proto cleanup (immediate, before Phase 2)

**Delete:**
- `crates/pane-proto/src/polarity.rs` — unused marker traits
- `crates/pane-proto/src/state.rs` — runtime state machine (replaced by session types)
- `crates/pane-proto/src/server/` — entire directory (ServerVerb, TypedView, Set/Unset, route.rs, roster.rs, views.rs)
- `crates/pane-proto/src/widget.rs` — server-rendered widgets (Phase 7 concern)
- `crates/pane-proto/tests/typed_views.rs` — tests for deleted code

**Modify:**
- `crates/pane-proto/src/lib.rs` — remove all deleted module declarations and re-exports
- `crates/pane-proto/Cargo.toml` — remove `optics = "0.3"` dependency
- `crates/pane-proto/src/message.rs` — extract `PaneId` to its own file, remove `PaneRequest`, `PaneEvent`, `RouteMessage`, `PaneKind` (these will be redesigned as session-typed protocol enums)
- `crates/pane-proto/src/attrs.rs` — remove `PaneMessage<T>`. Keep `AttrValue` temporarily (useful for filesystem/store contexts, will move to pane-store-client later)
- `crates/pane-proto/src/cell.rs` — remove from pane-proto. This moves to pane-comp temporarily (it's used by the renderer) and eventually to pane-ui. The cell grid is a client-side rendering concern, not a wire type.

**Rewrite:**
- `crates/pane-proto/tests/roundtrip.rs` — strip to only test surviving types (event, tag, color, PaneId, wire framing). Remove PaneRequest/PaneEvent/PaneMessage strategies and state machine tests.

**After cleanup, pane-proto contains:**
```
src/
  lib.rs
  wire.rs      # framing (unchanged)
  event.rs     # input types (unchanged)
  tag.rs       # tag line types (unchanged)
  color.rs     # color palette (unchanged)
  id.rs        # PaneId (extracted)
  attrs.rs     # AttrValue only (PaneMessage removed)
tests/
  roundtrip.rs # (reduced)
```

This is intentionally minimal. The protocol definitions (`protocol.rs`, `heartbeat.rs`) come in Phase 2 after pane-session exists, because they depend on its types.

### Phase 2: pane-session creation + pane-proto protocol defs

**Create** `crates/pane-session/` with the session type machinery described in section 5.

**Add to pane-proto:**
- `src/protocol.rs` — session type definitions for the pane protocol, using `pane-session` primitives:
  ```rust
  // Conceptual — actual syntax depends on pane-session design
  type PaneProtocol = Send<Hello, Offer<PaneOps>>;
  type PaneOps = enum {
      CreatePane(Send<CreateReq, Recv<CreateResp, Offer<PaneOps>>>),
      UpdateTag(Send<TagLine, Offer<PaneOps>>),
      Close(End),
  };
  ```
- `src/heartbeat.rs` — `Heartbeat(u64)` / `HeartbeatAck(u64)` message types.
- Redesigned message enums that work within session type contexts.

**Update Cargo.toml (workspace):**
```toml
[workspace]
resolver = "2"
members = [
    "crates/pane-proto",
    "crates/pane-session",
]
```

**pane-comp stays dormant.** Not in workspace members. The existing code is a Phase 4 concern. It still compiles independently (it depends on pane-proto's surviving types) but doesn't need to until Phase 4.

### Phase 3: pane-notify + pane-app

Create `crates/pane-notify/` and `crates/pane-app/`. These are new crates — nothing to migrate, everything to build.

### Phase 4: pane-comp rewrite

When the time comes, `pane-comp` gets rewritten from scratch on the session-typed foundation:
- calloop event loop (not thread::sleep)
- Session-typed pane protocol server
- Per-pane server threads (three-tier model)
- Layout tree
- Input dispatch
- Chrome rendering (glyph atlas carries forward)

The existing `main.rs` and `pane_renderer.rs` are discarded. `glyph_atlas.rs` is the only survivor, and even it needs significant rework (GPU upload, shared memory).

---

## 7. What the Cell Grid Decision Means

This deserves explicit treatment because it's the biggest structural change.

The current codebase assumes the compositor renders text from cell data — the `CellGrid` model. The client sends `WriteCells { cells: CellRegion }` and the compositor draws the glyphs. This is how terminal emulators traditionally work (the terminal is inside the display server's process).

The architecture spec commits to client-side rendering. Each pane renders its own buffer. The compositor composites those buffers. This is the Wayland model, and it's the same model BeOS used (applications drew into their own bitmaps; app_server composited).

This means:
1. **pane-shell renders its own text.** The PTY bridge + text renderer lives in the client process, not the compositor. pane-shell uses pane-ui's text rendering infrastructure (glyph atlas, etc.) to produce pixels, then submits a wl_surface buffer.
2. **Cell/CellAttrs/CellRegion move to pane-ui**, where they describe the client-side text buffer's internal representation — not a wire type.
3. **The compositor never sees cell data.** It sees opaque buffers (shared memory or DMA-BUF).
4. **Tag line is the exception.** The compositor renders tag lines (it owns the chrome). Tag content travels through the protocol as `TagLine` structs; the compositor renders them using its own text engine.

The glyph atlas code currently in pane-comp actually belongs in two places:
- A version in pane-comp for rendering tag lines and chrome text
- A version in pane-ui for client-side text rendering

These could share an underlying library crate, or pane-ui could own the glyph atlas and pane-comp could use pane-ui as a dependency for its chrome rendering. That's a Phase 4 design decision.

---

## 8. Build Sequence Alignment

| Phase | Crate Work | Deliverable |
|---|---|---|
| **Phase 1** (now) | Clean pane-proto: delete stale code, extract PaneId, remove optics dep | Minimal, correct wire types crate |
| **Phase 2** (next) | Create pane-session; add protocol defs to pane-proto | Session-typed conversation over unix socket, crash-safe |
| **Phase 3** | Create pane-notify, pane-app | Looper + handler + routing + filesystem notification |
| **Phase 4** | Rewrite pane-comp from scratch; create pane-shell | First pixels with session-typed protocols |
| **Phase 5** | Add layout tree, input binding to pane-comp; create pane-input | Tiling desktop with multiple shells |
| **Phase 6** | Create pane-roster, pane-store, pane-store-client, pane-watchdog | Infrastructure servers |
| **Phase 7** | Create pane-ui, pane-text, pane-fs | Rich rendering, text manipulation, FUSE |

Phase 1 is a half-day of deletion. Phase 2 is the hard part — the session type implementation determines everything that follows.

---

## 9. Risk: What If We're Wrong About Cell Grid?

The spec commits to client-side rendering. But there's a pragmatic question: for pane-shell specifically, rendering text in the compositor (the CellGrid model) would be simpler and faster to get running. A terminal emulator that sends cell data to the compositor can work without any client-side rendering infrastructure.

If the goal is "pane-shell working in pane-comp" as fast as possible (month four target), the CellGrid path gets there with less code. The client-side rendering path requires building pane-ui's text engine first.

My recommendation: the spec is right to commit to client-side rendering. The CellGrid model creates a permanent two-world problem — some panes are compositor-rendered, some are client-rendered, and the compositor must maintain both code paths forever. BeOS didn't do this. App_server composited bitmaps; it never rendered application content. The temporary convenience of CellGrid creates permanent architectural debt.

But if schedule pressure mounts, CellGrid-as-temporary-bridge is a defensible compromise. Ship a CellGrid-based pane-shell to get dogfood feedback, then migrate to client-side rendering when pane-ui exists. Just don't let the temporary path become permanent. Flag it with a loud TODO.

---

## 10. Summary: The Three-Sentence Version

Delete everything in pane-proto that assumes a central router, a runtime state machine, or compositor-rendered body content. Create pane-session as the custom session type crate — this is the Phase 2 critical path. The existing pane-comp is a demo, not a foundation; Phase 4 builds the real compositor on the session-typed protocols.
