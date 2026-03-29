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

### Requirement: Semantic filesystem tree
Each pane SHALL be exposed under `/pane/<id>/` with a tree structure that presents semantic interfaces, not implementation details. The filesystem exposes the abstraction level relevant to the consumer:

- `tag` — the tag line content (plain text, read/write)
- `body` — the body content at the semantic level (for a shell: command output; for an editor: file content; plain text, read/write)
- `attrs/` — directory of typed attributes, one file per attribute (pane type, title, dirty state, working directory)
- `ctl` — control interface (line commands, write-only)
- `event` — event stream (JSONL, read-only, blocking)

The tree does not expose rendering internals (glyph data, buffer state, GPU resources). A script reading `body` gets the content a human would see, at the semantic level the content operates at.

**Compositional equivalence** (architecture §2): when panes are composed, pane-fs reflects the composition structure as directory nesting. A split containing panes A and B appears as a directory with its own `attrs/` (orientation, ratio) and child entries `A/`, `B/`. Independent panes are top-level entries; composed panes are nested under their container. The filesystem tree mirrors the layout tree. Tools that walk `/pane/` see composition structure directly.

#### Scenario: Tag as plain text
- **WHEN** `cat /pane/1/tag` is executed
- **THEN** the plain text tag line content SHALL be returned without JSON wrapping

#### Scenario: Body as semantic content
- **WHEN** `cat /pane/1/body` is executed
- **THEN** the semantic body content SHALL be returned as plain text (not rendering internals, not buffer state)

#### Scenario: Attributes as individual files
- **WHEN** `cat /pane/1/attrs/title` is executed
- **THEN** the pane's title attribute SHALL be returned as a single value

#### Scenario: Events as JSONL
- **WHEN** `tail -f /pane/1/event` is executed
- **THEN** events SHALL arrive as one JSON object per line

### Requirement: Format per endpoint
Each filesystem node SHALL use the representation natural to its data. Plain text for text data (tag, body). One value per file for attributes (attrs/). Line commands for control files (ctl). JSONL for event streams (event).

#### Scenario: Attribute write
- **WHEN** `echo "new title" > /pane/1/attrs/title` is executed
- **THEN** the pane's title attribute SHALL be updated

### Requirement: Pane index
pane-fs SHALL expose a pane index at `/pane/index`.

#### Scenario: List all panes
- **WHEN** `cat /pane/index` is executed
- **THEN** a listing of all pane IDs SHALL be returned

#### Scenario: Directory listing
- **WHEN** `ls /pane/` is executed
- **THEN** each pane SHALL appear as a directory entry by its ID

### Requirement: Control file command syntax
The `ctl` file SHALL accept line-oriented commands. Each line is a command with optional arguments. For structured payloads (multi-property creation, complex operations), a JSON payload follows the command name.

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
pane-fs SHALL NOT expose the internal widget hierarchy of a pane. The scriptable surface of a pane is the set of attributes its handler declares via `PropertyDecl`. Internal rendering state (view trees, widget layouts, buffer positions) is opaque.

This is a deliberate divergence from BeOS, where `hey` could traverse into any application's view hierarchy (`get Frame of View "statusbar" of Window 0`). BeOS's deep traversal was powerful but fragile — scripts broke when applications rearranged their internal UI.

In pane, the scripting contract is: a pane exposes the attributes it chooses to expose. The level of abstraction is the pane, not the widget. If a pane wants internal structure to be scriptable, it declares those properties explicitly. The composer of the script and the author of the pane agree on a stable interface, rather than the script reaching into implementation details.

#### Scenario: No internal widget access
- **WHEN** a script attempts to access internal rendering state via pane-fs
- **THEN** only declared attributes SHALL be visible in `attrs/`
- **AND** the filesystem SHALL NOT expose view trees, widget hierarchies, or rendering internals

#### Scenario: Explicit property exposure
- **WHEN** a pane handler declares `PropertyDecl { name: "cursor-line", type: Int }`
- **THEN** `/pane/<id>/attrs/cursor-line` SHALL be readable
- **AND** scripts MAY depend on this property as part of the pane's stable scripting contract
