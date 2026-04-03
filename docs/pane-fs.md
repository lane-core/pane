## ADDED Requirements

### Requirement: FUSE service at /pane/
pane-fs SHALL expose pane state as a FUSE filesystem mounted at `/pane/`. pane-fs SHALL be a separate server process that communicates with other pane servers via the socket protocol. pane-fs is a translation layer — it converts FUSE operations into pane protocol messages. It is just another client of the pane servers. It has no special privilege and no server logic.

#### Scenario: Mount point available
- **WHEN** pane-fs starts
- **THEN** `/pane/` SHALL be accessible as a filesystem

#### Scenario: Separate process
- **WHEN** pane-comp crashes and restarts
- **THEN** pane-fs SHALL reconnect and continue serving the filesystem

### Requirement: FUSE-over-io_uring as baseline
pane-fs SHALL use FUSE-over-io_uring (Linux 6.14+) via a custom module built on the `io-uring` crate and `/dev/fuse` directly. No existing Rust FUSE library supports io_uring; the kernel interface is small (two io_uring subcommands + standard FUSE opcodes) and pane-fs needs only a bounded subset. This is not an optional optimization — it is a baseline requirement of the distribution.

#### Scenario: io_uring transport
- **WHEN** pane-fs initializes the FUSE session
- **THEN** it SHALL use io_uring for request/response transport, not read/write syscalls

### Requirement: Filesystem tier of the three-tier access model
pane-fs provides the filesystem access tier (~15-30μs per operation). The system offers three tiers:

| Tier | Mechanism | Latency | Use case |
|---|---|---|---|
| **Filesystem** | FUSE at `/pane/` | ~15-30μs per op | Shell scripts, inspection, configuration, event monitoring. Human-speed operations where 30μs is invisible. |
| **Protocol** | Session-typed unix sockets | ~1.5-3μs per op | Kit-to-server communication, rendering, input dispatch, bulk state queries. Machine-speed operations. |
| **In-process** | Kit API (direct function calls) | Sub-microsecond | Application logic within a pane-native client. No IPC, no serialization. |

If you'd be comfortable with 30μs latency and per-file granularity, use the filesystem. If you need machine-speed access with typed guarantees, use the protocol. Inside a pane-native client, the kit handles everything — the developer doesn't choose a tier.

#### Scenario: Shell script access
- **WHEN** a shell script reads `/pane/1/tag`
- **THEN** the operation SHALL complete with filesystem-tier latency (~15-30μs), not require protocol setup

### Requirement: Pane numbering and identity

Panes are numbered sequentially as they appear in the local
namespace. `/pane/1/`, `/pane/2/`, `/pane/3/`. The number is a
locally-assigned index — the name in this namespace, not the
identity. It is not stable across restarts.

The UUID is the globally-unique *identity*. It lives in a view
directory: `/pane/by-uuid/<uuid>/` contains a symlink to the
corresponding `/pane/<n>/`. Scripts that need cross-machine
stability use the UUID path. Humans and shell pipelines use
the number.

This follows Plan 9's /proc convention (PIDs are local indices,
not global identity) and Linux's /dev/disk/by-uuid pattern
(short name for use, UUID for stable reference).

When remote panes are mounted into the local namespace, they
receive local numbers alongside local panes. `/pane/7/` might
be local or remote — you don't care. That's namespace
transparency. `/pane/by-uuid/<uuid>/` lets you find the same
pane regardless of what number it was assigned on this machine.

`/pane/self/` is a context-dependent symlink to the calling
pane's directory (derived from the FUSE request's PID → owning
pane mapping). Analogous to `/proc/self/`.

#### Scenario: Numeric pane path
- **WHEN** `cat /pane/3/body` is executed
- **THEN** the semantic body content of pane 3 SHALL be returned

#### Scenario: UUID lookup
- **WHEN** `readlink /pane/by-uuid/550e8400-e29b-41d4-a716-446655440000` is executed
- **THEN** the result SHALL be a path to `/pane/<n>/` for the corresponding pane

#### Scenario: Self reference
- **WHEN** a process running inside pane 3 executes `cat /pane/self/attrs/title`
- **THEN** the title of pane 3 SHALL be returned

### Requirement: Semantic filesystem tree
Each pane SHALL be exposed under `/pane/<n>/` with a tree structure that presents semantic interfaces, not implementation details. The filesystem exposes the abstraction level relevant to the consumer:

- `tag` — title text (plain text, read/write). What appears in the title bar.
- `commands/` — command vocabulary (directory, read-only). One file per command. The discovery interface for the command surface.
- `body` — the body content at the semantic level (for a shell: command output; for an editor: file content; plain text, read/write)
- `attrs/` — directory of typed attributes, one file per attribute (pane type, signature, dirty state, working directory)
- `ctl` — control interface (line commands, write-only). Command invocation.
- `event` — event stream (JSONL, read-only, blocking)

The tree does not expose rendering internals (glyph data, buffer state, GPU resources). A script reading `body` gets the content a human would see, at the semantic level the content operates at.

**Compositional equivalence** (architecture §2): when panes are composed, pane-fs reflects the composition structure as directory nesting. A split containing panes A and B appears as a directory with its own `attrs/` (orientation, ratio) and child entries `A/`, `B/`. Independent panes are top-level entries; composed panes are nested under their container. The filesystem tree mirrors the layout tree. Tools that walk `/pane/` see composition structure directly.

#### Scenario: Tag as title text
- **WHEN** `cat /pane/1/tag` is executed
- **THEN** the title text SHALL be returned as plain text (e.g., "Editor — main.rs")

#### Scenario: Write tag
- **WHEN** `echo "Editor — lib.rs" > /pane/1/tag` is executed
- **THEN** the pane's title SHALL be updated

#### Scenario: Command discovery
- **WHEN** `ls /pane/1/commands/` is executed
- **THEN** one file per command SHALL be listed (e.g., `save`, `close`, `undo`)

#### Scenario: Command metadata
- **WHEN** `cat /pane/1/commands/save` is executed
- **THEN** the command's metadata SHALL be returned: description,
  keyboard shortcut, and group. Format: one key-value pair per
  line (`description: Save file`, `shortcut: Ctrl+S`,
  `group: File`).

#### Scenario: Command invocation via ctl
- **WHEN** `echo "save" > /pane/1/ctl` is executed
- **THEN** the pane SHALL receive `CommandExecuted { command: "save", args: "" }`
- **AND** this is the same effect as selecting "save" from the
  command surface (Alt+; prompt)

#### Scenario: Body as semantic content
- **WHEN** `cat /pane/1/body` is executed
- **THEN** the semantic body content SHALL be returned as plain text (not rendering internals, not buffer state)

#### Scenario: Attributes as individual files
- **WHEN** `cat /pane/1/attrs/signature` is executed
- **THEN** the pane's application signature SHALL be returned as a single value (e.g., "com.pane.editor")

#### Scenario: Events as JSONL
- **WHEN** `tail -f /pane/1/event` is executed
- **THEN** events SHALL arrive as one JSON object per line

### Requirement: Format per endpoint
Each filesystem node SHALL use the representation natural to its data. Plain text for text data (tag, body). Key-value pairs for command metadata (commands/<name>). One value per file for attributes (attrs/). Line commands for control files (ctl). JSONL for event streams (event).

#### Scenario: Attribute write
- **WHEN** `echo "true" > /pane/1/attrs/dirty` is executed
- **THEN** the pane's dirty attribute SHALL be updated

### Requirement: Pane index
pane-fs SHALL expose a pane index at `/pane/index`.

#### Scenario: List all panes
- **WHEN** `cat /pane/index` is executed
- **THEN** a listing of all panes SHALL be returned (number, UUID, signature, one per line)

#### Scenario: Directory listing
- **WHEN** `ls /pane/` is executed
- **THEN** each pane SHALL appear as a numbered directory entry (`1/`, `2/`, `3/`, ...)

### Requirement: Command vocabulary and control file

The `commands/` directory is the **discovery** interface — it
tells you what a pane can do. The `ctl` file is the **invocation**
interface — it does it. The command surface (Alt+; prompt in the
compositor, equivalent TUI in pane-headless) reads `commands/` to
populate its fzf-style completion list, and writes the user's
selection to `ctl`.

`commands/` is read-only from the filesystem perspective. The
command vocabulary is set by the pane's handler (via the Tag
builder at creation time and `Messenger::set_vocabulary()` for
dynamic updates). Bridge processes add commands through the
enrichment protocol (see `docs/legacy-wrapping.md` §3).

The `ctl` file SHALL accept line-oriented commands. Each line is
a command with optional arguments. For structured payloads
(multi-property creation, complex operations), a JSON payload
follows the command name.

Simple commands:
```
close
save
undo
```

Commands with arguments:
```
save /tmp/output.txt
goto 42
```

Structured payloads (for multi-property atomic operations):
```
create-command {"name":"save","description":"Save file","shortcut":"Ctrl+S","action":"client:save"}
```

This is the `hey AppName do ...` equivalent. Writing a command to `ctl` delivers it as `Message::CommandExecuted { command, args }` to the pane's handler.

#### Scenario: Simple ctl command
- **WHEN** `echo "close" > /pane/1/ctl` is executed
- **THEN** the pane SHALL receive `Message::CommandExecuted { command: "close", args: "" }`

#### Scenario: Ctl command with arguments
- **WHEN** `echo "save /tmp/out.txt" > /pane/1/ctl` is executed
- **THEN** the pane SHALL receive `Message::CommandExecuted { command: "save", args: "/tmp/out.txt" }`

#### Scenario: Ctl structured payload
- **WHEN** a JSON payload is written to `ctl` after a command name
- **THEN** the pane SHALL receive the command with the JSON as the args field, and MAY parse it for structured handling

### Requirement: Per-signature pane index
pane-fs SHALL expose a per-application-signature index at `/pane/by-sig/`. Each application signature that has running panes appears as a directory containing symlinks to those panes' directories.

This recovers the BeOS "count my application's windows" use case (`hey AppName count Window`) without requiring a full pane-store query.

#### Scenario: List panes by application
- **WHEN** `ls /pane/by-sig/com.example.editor/` is executed
- **THEN** symlinks to all panes owned by that application SHALL be listed

#### Scenario: Count application panes
- **WHEN** `ls /pane/by-sig/com.example.editor/ | wc -l` is executed
- **THEN** the count of panes owned by that application SHALL be returned

#### Scenario: New pane appears in index
- **WHEN** a new pane is created by `com.example.editor`
- **THEN** a symlink SHALL appear in `/pane/by-sig/com.example.editor/` without restart

### Requirement: Bulk attribute read
pane-fs SHALL support efficient bulk reads of all attributes for a pane. Reading `/pane/<id>/attrs.json` SHALL return all attributes as a single JSON object.

This addresses the round-trip concern: reading 10 individual attribute files requires 10 FUSE operations. Reading `attrs.json` is one operation.

#### Scenario: Bulk attribute read
- **WHEN** `cat /pane/1/attrs.json` is executed
- **THEN** a JSON object containing all attribute key-value pairs SHALL be returned
- **AND** the result SHALL be equivalent to reading each file in `attrs/` individually

### Requirement: Pane boundary principle
pane-fs SHALL NOT expose the internal widget hierarchy of a pane. The scriptable surface of a pane is the set of attributes its handler declares via `Attribute`. Internal rendering state (view trees, widget layouts, buffer positions) is opaque.

This is a deliberate divergence from BeOS, where `hey` could traverse into any application's view hierarchy (`get Frame of View "statusbar" of Window 0`). BeOS's deep traversal was powerful but fragile — scripts broke when applications rearranged their internal UI.

In pane, the scripting contract is: a pane exposes the attributes it chooses to expose. The level of abstraction is the pane, not the widget. If a pane wants internal structure to be scriptable, it declares those properties explicitly. The composer of the script and the author of the pane agree on a stable interface, rather than the script reaching into implementation details.

#### Scenario: No internal widget access
- **WHEN** a script attempts to access internal rendering state via pane-fs
- **THEN** only declared attributes SHALL be visible in `attrs/`
- **AND** the filesystem SHALL NOT expose view trees, widget hierarchies, or rendering internals

#### Scenario: Explicit property exposure
- **WHEN** a pane handler declares `Attribute { name: "cursor-line", type: Int }`
- **THEN** `/pane/<id>/attrs/cursor-line` SHALL be readable
- **AND** scripts MAY depend on this property as part of the pane's stable scripting contract

---

## Appendix: Configuration as Files

*(Merged from filesystem-config spec)*

## ADDED Requirements

### Requirement: Configuration as files
Server configuration SHALL be stored as files in well-known directories under `/etc/pane/<server>/`. Each config key SHALL be a separate file. The file content SHALL be the value. `/etc/pane/` is a writable directory on a persistent btrfs volume.

#### Scenario: Config value is file content
- **WHEN** `cat /etc/pane/comp/font` is executed
- **THEN** the output SHALL be the configured font name (e.g., "Iosevka")

#### Scenario: Config change via write
- **WHEN** `echo "JetBrains Mono" > /etc/pane/comp/font` is executed
- **THEN** the compositor SHALL detect the change via pane-notify and apply the new font

### Requirement: Config metadata in xattrs
Config files SHALL carry btrfs xattrs describing the entry: `user.pane.type` (string, int, float, bool), `user.pane.description` (human-readable description). Optional xattrs include `user.pane.range` (valid range for numeric values), `user.pane.options` (valid options for enum values). These xattrs are indexable by pane-store, making config keys queryable across the system (e.g., "which servers expose a font-size config?").

#### Scenario: Type metadata
- **WHEN** `getfattr -n user.pane.type /etc/pane/comp/font-size` is executed
- **THEN** the output SHALL indicate the type is "int"

#### Scenario: Config tooling
- **WHEN** a config tool reads `/etc/pane/comp/` and its xattrs
- **THEN** it SHALL have enough information to present appropriate input controls (text field for strings, slider for bounded ints, etc.)

### Requirement: Reactive configuration
All servers SHALL watch their config directories via pane-notify (the fanotify/inotify abstraction). Config changes SHALL take effect without server restart, without SIGHUP, and without manual reload commands. Servers cache config values at startup and update them on pane-notify events — no filesystem I/O in hot paths.

#### Scenario: Live font change
- **WHEN** the font config file is modified while the compositor is running
- **THEN** the compositor SHALL re-render all panes with the new font on the next frame

### Requirement: Discoverable configuration
All available config keys for a server SHALL be discoverable by listing its config directory. Every valid config key SHALL have a corresponding file, even if set to the default value.

#### Scenario: List all compositor config
- **WHEN** `ls /etc/pane/comp/` is executed
- **THEN** all compositor config keys SHALL be listed as files

### Requirement: Nix defaults with mutable overrides
At build time, Nix produces default config values in `/nix/store/<hash>-pane-config/`. On first boot or `pane-rebuild switch`, an activation script reconciles defaults against `/etc/pane/`:
- New keys: added with default values and xattr metadata
- Changed defaults: updated if the user has not modified the file; preserved if they have
- Removed keys: flagged for cleanup

User modification is tracked via `user.pane.modified` xattr. Nix owns the defaults; the user owns the overrides.

#### Scenario: Upgrade preserves user config
- **WHEN** a system rebuild changes the default font from "Noto Sans" to "Inter"
- **AND** the user has previously written "Iosevka" to `/etc/pane/comp/font`
- **THEN** the activation script SHALL preserve "Iosevka" (user-modified) and NOT overwrite with "Inter"

#### Scenario: Upgrade adds new key
- **WHEN** a system rebuild introduces a new config key `accent-color` for the compositor
- **THEN** the activation script SHALL create `/etc/pane/comp/accent-color` with the Nix-specified default value and appropriate xattr metadata

### Requirement: Unified namespace with computed views

pane-fs SHALL present a unified namespace where local and remote panes are interleaved under `/pane/`. The directory hierarchy is a query system — every directory is a computed view (a filter predicate over the indexed pane state). See `docs/distributed-pane.md` §3 for the full design.

Core computed views:
- `/pane/` — all panes (local and remote), numbered sequentially
- `/pane/by-uuid/<uuid>/` — stable global identity (symlinks to `/pane/<n>/`)
- `/pane/by-sig/<signature>/` — filter by application signature
- `/pane/by-type/<type>/` — filter by pane type
- `/pane/local/` — filter to local instance only
- `/pane/remote/` — filter to remote instances only
- `/pane/remote/<host>/` — filter to a specific remote host
- `/pane/self/` — calling pane's own directory

These are all projections over the same indexed state. The
top-level numbered listing is the primary interface — short,
human-usable, the Plan 9 /proc convention. Remote panes
receive local numbers when mounted, appearing alongside local
panes transparently. `by-uuid` provides cross-machine stability
for programmatic reference. `local` and `remote` are discovery
views, not architectural boundaries.

#### Scenario: Unified listing
- **WHEN** `ls /pane/` is executed
- **THEN** both local and remote panes SHALL appear as directory entries

#### Scenario: Filtered listing
- **WHEN** `ls /pane/local/` is executed
- **THEN** only panes on the local instance SHALL appear

#### Scenario: Remote pane access
- **WHEN** `cat /pane/<uuid>/body` is executed for a remote pane
- **THEN** pane-fs SHALL route the read to the remote instance transparently
- **AND** the response latency SHALL reflect the network round-trip, not a hang

### Requirement: Core/full FUSE backend split

pane-fs SHALL support two FUSE backends:

**Core (portable):** Standard FUSE via libfuse (Linux) or FUSE-T (Darwin). This is the default for headless deployments on any unix-like.

**Full (Pane Linux):** FUSE-over-io_uring (Linux 6.14+) with per-CPU request queues. This is the baseline for the full Pane Linux distribution.

Both backends expose the same `/pane/` namespace with the same semantics. The difference is performance: io_uring halves the per-operation overhead and eliminates concurrency bottlenecks.

**Darwin backend: FUSE-T.** FUSE-T emulates FUSE over NFS in userspace — no kernel extension (kext) required. This is Apple-proof: it does not depend on kext loading, which Apple has been progressively restricting. pane-fs is a synthetic filesystem with lightweight operations, so the NFS translation layer is invisible to consumers.

#### Scenario: FUSE backend selection
- **WHEN** pane-fs is built on Pane Linux
- **THEN** the io_uring backend SHALL be used

#### Scenario: FUSE on Darwin
- **WHEN** pane-fs is built on Darwin
- **THEN** the FUSE-T backend SHALL be used

### Requirement: Remote namespace mounting

pane-fs SHALL support transparent access to remote pane instances. Remote pane metadata is cached locally via pane-store's federation index, updated asynchronously via change notifications over the protocol. Reads go through the cache; writes route over TcpTransport to the owning instance.

Connections to remote hosts SHALL be established lazily on first access, not at mount time. If a remote host is unreachable, the filesystem operation SHALL return an error (ECONNREFUSED, ETIMEDOUT) rather than blocking indefinitely.

#### Scenario: Lazy remote connection
- **WHEN** a script first reads `/pane/remote/cloud-1/` after pane-fs starts
- **THEN** pane-fs SHALL establish a TcpTransport connection to `cloud-1`
- **AND** subsequent accesses SHALL reuse the connection

#### Scenario: Remote host unavailable
- **WHEN** a remote host is unreachable
- **THEN** reads of cached metadata SHALL succeed (stale data, marked as such)
- **AND** writes SHALL return an error immediately, not block
