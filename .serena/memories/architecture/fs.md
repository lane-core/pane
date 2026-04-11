---
type: architecture
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [pane-fs, namespace, FUSE, attributes, monadic_lens, optics, snapshot, scriptability, ctl, panenode, arcswap]
related: [decision/panefs_query_unification, decision/observer_pattern, decision/headless_strategic_priority, analysis/verification/fs_scripting, analysis/optics/panefs_taxonomy]
agents: [pane-architect, plan9-systems-engineer, optics-theorist]
---

# pane-fs Architecture

## Summary

pane-fs is the filesystem namespace crate — a strategic but currently sparse implementation that projects pane state into a `/pane/` synthetic filesystem for scripting and inspection. The crate embodies a unifying vision: **directories ARE queries, observable state lives as filesystem attributes** (per `decision/panefs_query_unification` and `decision/observer_pattern`), and scriptability happens through Plan 9-style ctl files and optics-powered attribute reads. 

The runtime is mostly stubs. Only the optics bridge (`AttrReader/AttrSet`) and directory model (`PaneEntry`) are real. No FUSE integration exists today. The crate contains just 3 source files (~240 LOC including tests), 5 passing tests, and explicit queuing in the status memo for: snapshot synchronization (ArcSwap), `PaneNode` trait, ctl parsing module, FUSE integration, and `#[derive(Scriptable)]` macro. 

The spec is complete in `docs/architecture.md` § "Namespace (pane-fs)". The design is ambitious: each `/pane/<id>/` directory carries monadic lenses that simultaneously serve read (through optics-derived `AttrReader` views) and write (through ctl commands). Per `decision/headless_strategic_priority`, this filesystem IS what makes Plan 9-style scriptability and cross-machine observability real — it's the foundational model for headless distribution.

## Modules

**`src/lib.rs`** (20 LOC) — Crate root with module declarations. Documents the namespace vision: per-pane directories under `/pane/` with structured entries (tag, body, attrs/*, ctl). Defines design heritage (Plan 9 /proc, rio /dev/wsys, BeOS hey scripting). No implementation.

**`src/namespace.rs`** (97 LOC) — `PaneEntry<S>` struct modeling a registered pane. Holds id, tag, state snapshot (clone-able, looper-updated), and an `AttrSet<S>`. Two methods: `read_attr(name)` for FUSE reads, `update_state(s)` for looper snapshots. Two tests covering snapshot reads and updates. This is the only real type in the crate today.

**`src/attrs.rs`** (160 LOC) — Optics bridge between pane-proto's effectful `MonadicLens<S,A>` (read/write/parse) and pane-fs's snapshot-only `AttrReader<S>` (read-only). Core types: `AttrValue` (newtype String, serialized for FUSE), `AttrReader<S>` (type-erased closure-based reader), `AttrSet<S>` (HashMap of readers, names() for readdir). Three tests: reader single-attribute read, attrset multi-attribute serve, attrset name enumeration. Intentionally separate from pane-proto's version — this is read-only HashMap snapshot access tuned for FUSE, not the writer-monad mutation layer.

## PaneNode Trait Status

**Queued for Phase 1** per status memo. No stub today. Will model a directory in the `/pane/` tree (either concrete — `/pane/<id>/` — or computed — `/pane/by-sig/`). Should support: `read_child(name)` → Option<PaneNode>, `list_children()` → Vec<&str>, `read_file(name)` → Vec<u8>, `stat()` → FUSE Attr. Design requirement from `decision/panefs_query_unification`: directories are computed views (predicates on pane state). No inheritance hierarchy — compositional, stateless methods on snapshots.

## Namespace Model

**Path structure** (from `docs/architecture.md`):

```
/pane/json              all panes as JSON array
/pane/<id>/             pane directory (id = local sequential index)
  tag                   title text (Lens, read-write)
  body                  content (Lens, read-write)
  attrs/                attribute directory
    <name>              individual attribute (Getter, read-only)
    json                all attrs from one snapshot (bulk Getter)
  ctl                   line-oriented command channel (write-only)
  json                  full pane state object
/pane/by-uuid/<uuid>/   view directory (symlink to /pane/<id>/)
/pane/by-sig/<sig>/     query directory (filtered by signature)
/pane/remote/           query directory (topology == remote)
```

**Types representing the tree:**

- **`PaneEntry<S>`** (namespace.rs:16–34) — Concrete entry for `/pane/<id>/`. Generic over handler state S. Holds id (u64), tag (String), attrs (AttrSet<S>), state (S clone snapshot). Methods: read_attr, update_state.

- **`AttrValue`** (attrs.rs:31–38) — Newtype `(String)`, serialized for FUSE. From optics view via Display impl. Semantics: text-based `/pane/<id>/attrs/<name>` file content.

- **`AttrReader<S>`** (attrs.rs:47–73) — Type-erased attribute reader. Wraps a boxed closure `Fn(&S) -> AttrValue`. Constructed from a view function and Display type. No mutable state — pure read from snapshot.

- **`AttrSet<S>`** (attrs.rs:77–107) — HashMap<&'static str, AttrReader<S>>. Methods: add (builder), read (lookup), names (readdir). The attribute namespace for one pane.

**No explicit type for query directories** (`/pane/by-sig/`, `/pane/remote/`) yet. Will be computed in PaneNode impl. Filtering is a closure captured in PaneNode construction.

## Ctl Parsing Status

**Queued for Phase 1** per status memo. No implementation today. Design is specified: "line-oriented command interface" with synchronous blocking writes. Multi-line writes process sequentially, stop on first error.

**Spec from `docs/architecture.md`:**
- Write handler sends (command, oneshot_tx) to looper via calloop channel.
- Looper processes command, executes effects, publishes snapshot, sends result.
- Error reporting via FUSE errno: EINVAL (syntax), EIO (handler panic), ENXIO (pane exited), ETIMEDOUT (5s).
- Commands are stateful: e.g., "cursor 42", "set-tag \"foo\"", "goto". Consumed by Handler via monadic lens setters.

**Grammar notes (implicit from optics layer):**
- Each command name maps to a MonadicLens<S, A> via its `parse` function.
- Argument syntax is value-type-specific (set_tag: quoted string, cursor: integer, etc.).
- Multi-arg commands case-analyzed by handler (future struct-based API).
- Per `analysis/verification/fs_scripting`: structured ctl commands better than dynamic message fields.

## Snapshot Synchronization Status

**Queued for Phase 1** per status memo: "ArcSwap-based snapshot synchronization". Greenfield today — no hooks.

**Design from `docs/architecture.md`:**
- Looper publishes a Clone'd state snapshot after each dispatch cycle (batch end, before Notifications phase).
- FUSE threads read from snapshot via ArcSwap (zero-contention atomic swap).
- Per-pane consistency: all attributes in one FUSE operation from same dispatch cycle.
- `attrs/json` extends to bulk reads — one FUSE read, one snapshot, all attrs as JSON object.

**Implementation pattern (future):**
- `PaneEntry<S>` should wrap S in ArcSwap<Arc<S>>.
- `update_state(s)` becomes `self.state_arc.swap(Arc::new(s))`.
- `read_attr(name)` clones the arc: `let snap = Arc::clone(&self.state_arc.load()); self.attrs.read(name, &snap)`.
- Looper calls `update_state()` at batch end → immediate visibility to FUSE without blocking.

## FUSE Integration Status

**Completely unimplemented** per status memo and code review. Confirmation: no FUSE crate dependency in Cargo.toml, no integration code or stubs. Filesystem mounting is purely aspirational.

**Design stubs from `docs/architecture.md`:**
- FUSE operations: read, readdir, write.
- `open(path)` resolves via PaneNode tree walk.
- `read(fd)` → calls node's read_file(), returns Vec<u8>.
- `readdir(fd)` → calls node's list_children(), returns DirEntry vector.
- `write(fd, buf)` → ctl dispatch: send (command, oneshot_tx) to looper, block on oneshot_rx, return bytes consumed or errno.
- FUSE permission model: ReadWrite (0660) from AttrAccess::ReadWrite, ReadOnly (0440) from ReadOnly/Computed (pane-proto/src/monadic_lens.rs:69–74).

**Missing:** fuse-sys/fuse3 crate integration, fd→node mapping, directory enumeration logic, permission enforcement, error translation.

## `#[derive(Scriptable)]` Macro Status

**Queued for Phase 1** per status memo. No scaffolding today. Intended to: 
- Accept handler struct with MonadicLens attributes.
- Generate `AttrSet::new()` with all lenses registered as `AttrReader`s.
- Emit a `ctl_dispatch` function that maps command names to lens parse/set functions.
- Derive `PropertyInfo` metadata (optics type, access mode, description).

**Macro target:**
```rust
#[derive(Scriptable)]
struct MyHandler {
    #[monadic_lens]
    state: MyState,
}
```

Should produce AttrSet + dispatch logic, similar to BeOS `GetSupportedSuites()/ResolveSpecifier()` pattern.

## Five Tests

All in `namespace.rs` and `attrs.rs`:

1. **`attr_reader_reads_from_state_ref`** (attrs.rs:119–129) — Construct reader from view closure, call read(), assert string-serialized value. Tests: view capture, Display conversion.

2. **`attr_set_serves_multiple_attributes`** (attrs.rs:131–147) — Build AttrSet with two readers, read by name, nonexistent returns None. Tests: HashMap add/read, multiple attributes, missing key.

3. **`attr_set_lists_names`** (attrs.rs:149–158) — Build AttrSet, call names(), assert Vec matches inserted keys. Tests: readdir enumeration.

4. **`pane_entry_reads_attrs_from_snapshot`** (namespace.rs:47–67) — Build PaneEntry with status/uptime attributes, call read_attr() twice, assert values match snapshot. Tests: PaneEntry attr bridging.

5. **`pane_entry_reflects_state_updates`** (namespace.rs:69–95) — Build PaneEntry, read attr, call update_state() with new snapshot, read again, assert new value. Tests: looper update contract.

**What's real:** attribute snapshot read model (optics→filesystem bridge). **What's stubbed:** FUSE operations, ctl parsing, batch synchronization, computed directories.

## Invariants & Design Principles

From `docs/architecture.md` and decisions:

1. **Snapshot consistency:** Per-FUSE-operation atomicity via Clone. Reads never block the looper. Cross-attribute reads use bulk json to maintain consistency.

2. **Optic classification** (from `analysis/optics/panefs_taxonomy.md`):
   - `/pane/<id>/tag`, `/pane/<id>/body` → **Lens** (PutGet/GetPut/PutPut laws within snapshot)
   - `/pane/<id>/attrs/<name>` → **Getter** (read-only, AffineFold if filtering)
   - `/pane/<id>/ctl` → **NOT an optic** (write-only command channel, effectful/non-idempotent)
   - Path composition: `/pane/3/tag` = Iso . AffineTraversal . Lens = AffineTraversal

3. **Filesystem = scripting:** Plan 9 model: ctl writes block until looper processes and publishes updated snapshot. Read after write sees effect. Validated against 10 real BeOS scripting scenarios (analysis/verification/fs_scripting.md: 7/10 clean, 1/10 better, 2/10 design attention needed).

4. **Computed views:** `/pane/by-sig/`, `/pane/remote/` are predicates on pane state, not stored. Tree is entirely computed. Unifies BFS queries (indices → live result sets) with Plan 9 synthetic filesystems.

5. **Observable state via attributes, not messaging:** Replaces BeOS StartWatching/SendNotices. Reasons (decision/observer_pattern): initial-value problem solved (read then watch), crash recovery (state persists), scriptability (pane-notify watch without app cooperation), C1 alignment (separate watches), minimal infra.

## See Also

- **`docs/architecture.md`** § "Namespace (pane-fs)" — Full spec: paths, snapshot model, ctl writes, namespace as test surface.
- **`decision/panefs_query_unification`** — Vision: directories ARE queries, hierarchy is view composition, BFS+Plan9 unification.
- **`decision/observer_pattern`** — Why filesystem attributes, not messaging. Replaces BeOS StartWatching.
- **`decision/headless_strategic_priority`** — Filesystem IS the foundation for distributed/headless scriptability.
- **`analysis/verification/fs_scripting.md`** — Validation against 10 BeOS scripting scenarios. Outcome: 7/10 clean, needs ctl syntax definition and per-sig index.
- **`analysis/optics/panefs_taxonomy.md`** — Optic classification of every path. Guides FUSE permission derivation.
- **`pane-proto/src/monadic_lens.rs`** — Effectful optics layer (view pure, set returns effects). AttrReader/AttrSet types pane-fs depends on.
- **`status.md`** Phase 1 queue — snapshot sync (ArcSwap), PaneNode trait, ctl parsing, FUSE, Scriptable derive.
