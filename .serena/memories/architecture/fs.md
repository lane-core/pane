---
type: architecture
status: current
supersedes: []
created: 2026-04-11
last_updated: 2026-04-12
importance: high
keywords: [pane-fs, namespace, FUSE, attributes, monadic_lens, optics, snapshot, scriptability, ctl, panenode, arcswap, Dev, compositor, kernel, router, tags]
related: [decision/panefs_query_unification, decision/observer_pattern, decision/headless_strategic_priority, architecture/kernel, architecture/compositor, architecture/router, analysis/verification/fs_scripting, analysis/optics/panefs_taxonomy]
agents: [pane-architect, plan9-systems-engineer, optics-theorist]
---

# pane-fs Architecture

## Summary

pane-fs is the namespace layer — the universal projection
surface that every subsystem mounts into. It provides the
file protocol access path for the two-tier pattern
(file protocol + typed API) that is uniform across pane.

In the Plan 9 tradition: the namespace IS the interface.
pane-kernel registers devices, pane-compositor registers
window state, pane-app registers pane state, and pane-fs
presents all of it as a coherent synthetic filesystem under
`/pane/`. Everything observable or controllable through the
file protocol goes through pane-fs.

Currently sparse (3 files, ~240 LOC, 5 tests). The optics
bridge (AttrReader/AttrSet) and directory model (PaneEntry)
are real. FUSE integration, ctl parsing, computed views, and
subsystem mount points are queued for Phase 1.

## Role in the architecture

pane-fs is the namespace that binds all subsystems together:

pane occupies three mount points on the host filesystem:

| Host path | Provider | Role |
|---|---|---|
| `/srv` | pane-kernel / pane-session | Service posting (Plan 9 /srv directly) |
| `/dev/pane/` | pane-kernel | Devices from DeviceRegistry |
| `/pane/` | pane-fs | Pane state, compositor, computed views |

`/srv` and `/dev/pane/` are root-level namespace positions
because pane is a system layer, not an application. Services
go where services go, devices where devices go.

### Full namespace layout

| Path | Provider | Content |
|---|---|---|
| `/srv/<name>` | pane-session services | Posted service fds (clipboard, ime, plumb, etc.) |
| `/dev/pane/<name>` | pane-kernel | Individual Dev device (read/write) |
| `/dev/pane/keyboard` | pane-kernel | Input device |
| `/dev/pane/display` | pane-kernel | Display device |
| `/dev/pane/audio` | pane-kernel | Audio device |
| `/pane/<id>/` | pane-app | Pane state, attrs, ctl, tags |
| `/pane/<id>/tag` | pane-app | Title text (Lens) |
| `/pane/<id>/body` | pane-app | Content (Lens) |
| `/pane/<id>/attrs/<name>` | pane-app | Attribute (Getter) |
| `/pane/<id>/ctl` | pane-app | Command channel (write-only) |
| `/pane/<id>/tags` | pane-app | Command hierarchy for tab launcher |
| `/pane/comp/` | pane-compositor | Compositor state |
| `/pane/comp/windows/` | pane-compositor | Window list |
| `/pane/comp/windows/<id>/` | pane-compositor | Per-window state |
| `/pane/comp/windows/<id>/ctl` | pane-compositor | Window management commands |
| `/pane/comp/workspaces/` | pane-compositor | Workspace list and state |
| `/pane/by-sig/<sig>/` | pane-fs (computed) | Query: filter by signature |
| `/pane/by-type/<type>/` | pane-fs (computed) | Query: filter by type |
| `/pane/remote/` | pane-fs (computed) | Query: topology == remote |
| `/pane/local/` | pane-fs (computed) | Query: topology == local |

Each subsystem registers its namespace subtree with pane-fs.
pane-fs handles path resolution, FUSE integration, and
computed views. Subsystems implement the content.

### The two-tier pattern

Every subsystem exposes both access paths through pane-fs:

- **File protocol:** `cat /pane/3/tag`, `echo resize 0 0 800 600 > /pane/comp/windows/3/ctl`, `cat /dev/pane/keyboard`, `ls /srv`
- **Typed API:** Handler methods, Protocol messages, Dev traits

Both paths access the same underlying state. pane-fs owns
the file protocol path. The typed API path is direct (no
pane-fs involvement).

### Relationship to pane-kernel

pane-kernel provides the DeviceRegistry (devtab[]). pane-fs
mounts DeviceRegistry entries at `/dev/pane/`. When a pane
reads `/dev/pane/keyboard`, pane-fs resolves the path to the
Dev object and calls `dev.read()`. When a script writes to
`/dev/pane/display`, pane-fs calls `dev.write()`.

pane-kernel does NOT do path resolution. pane-fs does.
pane-kernel provides the device table; pane-fs provides the
namespace.

Per-pane device visibility: the compositor provides a device
view predicate per pane (HashSet of visible device names).
pane-fs applies this predicate when resolving paths under
`/pane/dev/` in a pane's context.

### Relationship to pane-compositor

pane-compositor registers its state tree with pane-fs:
- Window list, per-window state (title, geometry, workspace,
  focus)
- Workspace list and active workspace
- wctl commands via ctl files

This makes window management scriptable:
`echo "workspace 2" > /pane/comp/windows/3/ctl`

The compositor's tab/launcher reads the command hierarchy
from `/pane/<id>/tags` — pane-fs provides this file, sourced
from the pane's registered tag tree.

### Relationship to pane-router

pane-router's rules may be observable/configurable through
pane-fs in the future. The router's audit log could be
readable from the namespace. This is deferred.

## Namespace model

**Directories ARE queries** (per `decision/panefs_query_unification`).
Each directory is a computed view — a predicate on pane
state. The tree is computed, not stored.

- `/pane/by-sig/com.pane.agent/` = query `signature == 'com.pane.agent'`
- `/pane/remote/` = query `topology == remote`
- `/pane/local/` = query `topology == local`

Local + remote unified is the **default**. Filtering is
another computed directory view.

**Observable state via filesystem attributes, not messaging**
(per `decision/observer_pattern`). Replaces BeOS
StartWatching/SendNotices. Reasons: initial-value problem
solved (read then watch), crash recovery, scriptability,
minimal infrastructure.

## Modules (current implementation)

**`src/lib.rs`** (20 LOC) — Crate root. Documents the
namespace vision and design heritage (Plan 9 /proc, rio
/dev/wsys, BeOS hey scripting).

**`src/namespace.rs`** (97 LOC) — `PaneEntry<S>` struct:
id, tag, state snapshot, AttrSet. Methods: read_attr,
update_state. Two tests.

**`src/attrs.rs`** (160 LOC) — Optics bridge. `AttrValue`
(newtype String), `AttrReader<S>` (type-erased closure
reader), `AttrSet<S>` (HashMap of readers). Three tests.

## Per-pane directory structure

```
/pane/<id>/
  tag              title text (Lens, read-write via ctl)
  body             content (Lens, read-write via ctl)
  attrs/           attribute directory
    <name>         individual attribute (Getter, read-only)
    json           all attrs from one snapshot (bulk)
  ctl              line-oriented command channel (write-only)
  tags             command hierarchy tree (read-only, for launcher)
  json             full pane state as JSON
```

The `tags` file is new — it exposes the pane's declared
command vocabulary as a navigable hierarchy. The
compositor's tab/launcher reads this to populate the command
palette. Composable with unix tools: `cat /pane/3/tags | fzf`.

## Optic classification

From `analysis/optics/panefs_taxonomy`:

- `/pane/<id>/tag`, `/pane/<id>/body` → **Lens** (PutGet/GetPut/PutPut)
- `/pane/<id>/attrs/<name>` → **Getter** (read-only)
- `/pane/<id>/ctl` → **NOT an optic** (write-only, effectful)
- `/pane/<id>/tags` → **Getter** (read-only command tree)
- `/pane/dev/<name>` → **Dev trait** (open/read/write/close, not an optic)
- `/pane/comp/windows/<id>/*` → **Getter** (compositor state projection)
- Path composition: `/pane/3/tag` = Iso ∘ AffineTraversal ∘ Lens = AffineTraversal

## Snapshot synchronization (queued)

ArcSwap-based. Looper publishes Clone'd state snapshot after
each dispatch cycle. FUSE threads read via ArcSwap
(zero-contention atomic swap). Per-pane consistency: all
attributes in one FUSE operation from same dispatch cycle.

## Ctl parsing (queued)

Line-oriented command interface. Write handler sends
(command, oneshot_tx) to looper via calloop channel. Looper
processes, executes effects, publishes snapshot, sends result.
Error reporting via FUSE errno: EINVAL (syntax), EIO (panic),
ENXIO (pane exited), ETIMEDOUT (5s).

## PaneNode trait (queued)

Models a directory in the `/pane/` tree. Must support
subsystem registration — pane-kernel registers `/pane/dev/`,
pane-compositor registers `/pane/comp/`, pane-app registers
`/pane/<id>/`. Compositional, stateless methods on snapshots.

## FUSE integration (queued)

Completely unimplemented. No fuse dependency in Cargo.toml.
Design specified in docs/architecture.md. Operations: read,
readdir, write (ctl dispatch).

## `#[derive(Scriptable)]` macro (queued)

Generates AttrSet + ctl dispatch from handler struct with
MonadicLens attributes. Also generates the tags hierarchy
from the declared command vocabulary.

## Invariants

1. **Snapshot consistency:** Per-FUSE-operation atomicity via
   Clone. Reads never block the looper.
2. **Filesystem = scripting:** ctl writes block until looper
   processes. Read after write sees effect.
3. **Computed views:** directories are predicates on state.
   Tree is entirely computed, not stored.
4. **Observable state:** filesystem attributes, not messaging.
5. **Namespace = universal interface:** every subsystem mounts
   into `/pane/`. File protocol + typed API both access the
   same state.

## Provenance

Initial design established early in pane development.
Updated 2026-04-12 to reflect the broader namespace role
after pane-kernel (DeviceRegistry mount), pane-compositor
(window state projection, tags for launcher), and
pane-router (signal-flow policy) were designed. The
two-tier pattern (file protocol + typed API) and exokernel
framing clarified pane-fs as the universal projection
surface.

## See also

- `architecture/kernel` — DeviceRegistry → `/pane/dev/`
- `architecture/compositor` — window state → `/pane/comp/`
- `architecture/router` — future: rules in namespace
- `decision/panefs_query_unification` — directories ARE queries
- `decision/observer_pattern` — attrs not messaging
- `decision/headless_strategic_priority` — fs IS the foundation
- `analysis/optics/panefs_taxonomy` — optic classification
- `analysis/verification/fs_scripting` — scripting validation
- `docs/architecture.md` § "Namespace (pane-fs)" — full spec

**Files:** `crates/pane-fs/src/{lib,namespace,attrs}.rs`
(~240 LOC, 5 tests)
