## Context

After building pane-proto and speccing pane-comp, a deep design review examined our reference systems (BeOS, Plan 9, NeXTSTEP) and identified gaps between the architecture spec and the actual design intent. A code review of pane-proto found critical state machine bugs and type design issues. Further study of sequent calculus (Binder et al.'s "Grokking the Sequent Calculus," Mangel/Melliès/Munch-Maccagnoni's duploid framework, Munch-Maccagnoni's "Dissection of L") identified a principled approach to protocol composition via Value/Compute polarity. This change captures all architectural refinements, renames, and code fixes, and rewrites the architecture spec as a single coherent document.

## Goals / Non-Goals

**Goals:**
- Rewrite architecture spec as single coherent document (not incremental deltas)
- Capture all architectural decisions as durable design commitments
- Fix pane-proto's state machine, CellRegion, and wire type issues
- Introduce Value/Compute polarity (from sequent calculus/CBPV) as the formal grounding for protocol composition
- Rename plumb/plumber to route/router throughout
- Redesign state machine for multi-pane-per-connection
- Add inter-server protocol (ServerVerb + attrs with typed views)
- Establish the filesystem-as-interface principle as a core design pillar
- Make the platform commitment (Linux-only, s6/runit) explicit
- Clarify compositor↔router relationship and pane-input status

**Non-Goals:**
- Implementing pane-notify, pane-fs, or the config system (those are future changes)
- Building the plugin discovery infrastructure
- Implementing the plumber multi-match UI
- Changing the pane-comp-skeleton proposal (it stays scoped to what it is)

## Decisions

### 1. PaneMessage attrs bag

Every protocol message wraps its typed core in a PaneMessage that carries an open key-value attributes bag:

```rust
struct PaneMessage<T> {
    core: T,
    attrs: Vec<(String, AttrValue)>,
}

enum AttrValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Bytes(Vec<u8>),
    Attrs(Vec<(String, AttrValue)>),  // nesting
}
```

Storage is Vec (ordered, duplicates allowed — matches BMessage semantics). Access via optics-style methods: `attr("key")` → `Option<&AttrValue>`, `attrs_all("key")` → iterator, `set_attr()`, `insert_attr()`. The `optics` crate is a dependency for composed access paths when nesting complexity justifies it.

### 2. FUSE at /srv/pane/

A separate `pane-fs` server (not built into pane-comp) that speaks pane protocol to the compositor and exposes state as a FUSE filesystem at `/srv/pane/`. Format per endpoint:

- `tag` — plain text (it's text)
- `body` — plain text read (the characters)
- `cells` — JSON (full cell fidelity with colors/attrs)
- `ctl` — line commands write, state read
- `attrs` — JSON (the attrs bag)
- `event` — JSONL stream
- `index` — JSON array
- `plumb/send` — JSON (or plain text shorthand)
- `plumb/<port>` — JSONL stream
- `config/` — see decision 8

### 3. Linux-only

Target Linux exclusively, track latest stable kernel. Unlocks: mount namespaces, user namespaces, fanotify, xattrs, memfd, pidfd, seccomp.

Init system: s6 or runit (supervision tree model). Not systemd. pane infrastructure services are managed by the init system. Desktop apps are managed by pane-roster.

Filesystem: xattr support required (ext4/btrfs/XFS/bcachefs). Advanced features (snapshots, subvolumes, CoW) available through an abstraction layer when the filesystem provides them.

### 4. pane-notify

Internal crate abstracting over Linux filesystem notification:

- fanotify with `FAN_MARK_FILESYSTEM` for mount-wide watches (pane-store bulk xattr tracking)
- inotify for targeted watches (specific directories, config files, plugin dirs)
- Unified event stream integrating into calloop
- Consumers request by scope; pane-notify picks the right kernel interface

### 5. pane-roster hybrid model

Infrastructure servers (pane-comp, pane-plumb, pane-store, pane-input, pane-fs) are supervised by s6/runit. They register with pane-roster on startup — roster is the directory, not the supervisor.

Desktop apps (shells, editors, user-launched programs) are supervised by pane-roster directly. Roster handles launch, crash restart, session save/restore.

Service registry: apps register `(content_type_pattern, operation_name, description)` tuples. The plumber queries the registry for multi-match scenarios.

### 6. Plumber multi-match

Single match: auto-dispatch (current behavior). Multiple matches: spawn a transient floating pane (scratchpad) listing options as B2-clickable text lines. Plumber rules take priority over registered services.

### 7. Filesystem-based plugin discovery

Servers scan well-known directories for add-ons:
- `~/.config/pane/translators/` — content translators (type sniffing, format conversion)
- `~/.config/pane/input/` — input method add-ons
- `~/.config/pane/plumb/rules/` — plumber rules (one file per rule)

pane-notify watches these directories. Adding/removing a file adds/removes the capability live.

### 8. Filesystem-as-configuration

Config values are files in well-known directories. File content is the value. xattrs carry metadata about the config entry (type, description, valid range). pane-notify makes it reactive — change a file, the server adapts. No config file parsers, no SIGHUP.

```
/etc/pane/comp/font       → content: "Iosevka", xattr user.pane.type: string
/etc/pane/comp/font-size  → content: "14", xattr user.pane.type: int, xattr user.pane.range: 6-72
```

Readable: `cat /etc/pane/comp/font`. Writable: `echo "JetBrains Mono" > /etc/pane/comp/font`. Discoverable: `ls /etc/pane/comp/`. Versionable: put `/etc/pane/` in git.

Also exposed via FUSE: `/srv/pane/config/` mirrors `/etc/pane/` with the same semantics.

### 9. Value/Compute polarity

Grounded in sequent calculus (Curien & Herbelin's λμμ̃) and Call-by-Push-Value (Levy). Protocol types have polarity:

- **Value** types are constructed by the sender and inspected by the receiver. Requests, cell data, route messages, attr values are all Values. They're data — you pick a variant and fill in fields.
- **Compute** types are defined by behavior when observed. Event handlers, protocol continuations are Computations. They're codata — you provide a handler for each observation.

This is expressed as marker traits (`Value`, `Compute`) that cost nothing at runtime but let the `Proto<A>` combinator enforce valid composition:

- `Value.and_then(Value)` — sequential, both produce results ✓
- `Value.and_then(Compute)` — get a value, then fire off work ✓
- `Compute.and_then(Compute)` — chain behaviors ✓
- `Compute.and_then(Value)` — must explicitly synchronize first ⚠

The compiler enforces the duploid's "three fourths" associativity rule. The fourth case requires explicit `await_completion()`.

Event handling has dual representations: the `PaneEvent` enum (data/Value — for exhaustive matching) and a `PaneHandler` builder (codata/Compute — for declarative dispatch). Both are available; the developer picks the style that fits.

The fundamental operation is the **cut**: `⟨request | handler⟩`. Protocol dispatch is a cut. Making cuts explicit enables pure-function testing of dispatch without I/O.

### 10. Inter-server protocol

All inter-server communication uses `PaneMessage<ServerVerb>` where:

```rust
enum ServerVerb { Query, Notify, Command }
```

The verb is the typed core. The attrs bag carries the payload. This is the BMessage model: one envelope, universal routing, convention-checked semantics.

Type safety is recovered at the kit layer via typed views and builders:

```rust
struct RouteCommand<'a> { msg: &'a PaneMessage<ServerVerb> }
impl RouteCommand {
    fn parse(msg: &PaneMessage<ServerVerb>) -> Result<Self, ProtocolError> { ... }
    fn data(&self) -> &str { ... }
}

// Builder enforces required fields at compile time
RouteCommand::build().data("parse.c:42").wdir("/src").into_message()
```

The wire format is universal (any server can forward/log/inspect). The access layer is typed (compiler catches missing fields). This gives BMessage's loose coupling with Rust's type safety.

### 11. Multi-pane per connection

A single client connection can own multiple panes. The state machine tracks a set:

```
Disconnected → Active { panes: HashMap<PaneId, PaneKind>, pending_creates: u32 }
```

Create increments pending_creates. activate(id, kind) inserts into the map. Close removes. Operations validated against pane map and kind. Most clients create one pane; complex clients (editor with splits) create several.

### 12. Rename plumb → route

All plumb/plumber terminology renamed to route/router:
- pane-plumb → pane-route
- PlumbMessage → RouteMessage
- TagPlumb → TagRoute
- /srv/pane/plumb/ → /srv/pane/route/

### 13. pane-input absorbed into pane-comp

Input handling (libinput, xkbcommon, key binding resolution) is a module within pane-comp, not a separate server. Every production Wayland compositor handles input in-process for latency reasons. Input method add-ons are separate processes connecting via Wayland IME protocols.

### 14. pane-proto code fixes

- **PendingCreate state**: `apply(Create)` on Connected transitions to PendingCreate. Only `activate()` transitions PendingCreate → Active. A second Create while PendingCreate is rejected.
- **CellRegion height**: Add explicit `height: u16` field. Validate `cells.len() == width as usize * height as usize` on construction.
- **PaneKind in Active**: `ProtocolState::Active { pane_id, kind }`. State machine rejects WriteCells/Scroll for Surface panes.
- **frame() truncation**: `payload.len().try_into::<u32>()` with error instead of silent `as u32`.
- **NamedKey::F range**: Newtype `FKey(u8)` with `TryFrom<u8>` validating 1-24.
- **Re-export frame/frame_length** from crate root.
- **Remove Serialize/Deserialize** from ProtocolState (it's local tracking, not a wire type).
- **Document Scroll delta**: positive = down (toward newer content), unit = rows.

## Risks / Trade-offs

**[PaneMessage wrapper adds indirection]** → Every protocol message is now `PaneMessage<PaneRequest>` instead of just `PaneRequest`. Serialization size grows (empty attrs vec still costs a length prefix). Mitigation: postcard encodes an empty vec as a single zero byte. Ergonomic cost is one `.core` access — manageable.

**[Filesystem-as-config is unusual]** → Developers expect TOML/YAML config files. The file-per-value model is unfamiliar. Mitigation: it's more *discoverable* than config files (ls shows you everything), more scriptable (echo/cat), and more reactive (live updates). The learning curve is front-loaded.

**[Plugin discovery via filesystem has security implications]** → Any file dropped in `~/.config/pane/translators/` gets loaded. Mitigation: translators could be sandboxed via seccomp/namespaces. Or they could be separate processes (executables) rather than shared libraries, avoiding code loading entirely. Design this when we build the translator system.
