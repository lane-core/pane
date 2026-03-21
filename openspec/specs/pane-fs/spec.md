## ADDED Requirements

### Requirement: FUSE service at /srv/pane/
pane-fs SHALL expose pane state as a FUSE filesystem mounted at `/srv/pane/`. pane-fs SHALL be a separate server process that communicates with other pane servers via the socket protocol. pane-fs is a translation layer — it converts FUSE operations into pane protocol messages. It is just another client of the pane servers. It has no special privilege and no server logic.

#### Scenario: Mount point available
- **WHEN** pane-fs starts
- **THEN** `/srv/pane/` SHALL be accessible as a filesystem

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
| **Filesystem** | FUSE at `/srv/pane/` | ~15-30μs per op | Shell scripts, inspection, configuration, event monitoring. Human-speed operations where 30μs is invisible. |
| **Protocol** | Session-typed unix sockets | ~1.5-3μs per op | Kit-to-server communication, rendering, input dispatch, bulk state queries. Machine-speed operations. |
| **In-process** | Kit API (direct function calls) | Sub-microsecond | Application logic within a pane-native client. No IPC, no serialization. |

If you'd be comfortable with 30μs latency and per-file granularity, use the filesystem. If you need machine-speed access with typed guarantees, use the protocol. Inside a pane-native client, the kit handles everything — the developer doesn't choose a tier.

#### Scenario: Shell script access
- **WHEN** a shell script reads `/srv/pane/1/tag`
- **THEN** the operation SHALL complete with filesystem-tier latency (~15-30μs), not require protocol setup

### Requirement: Semantic filesystem tree
Each pane SHALL be exposed under `/srv/pane/<id>/` with a tree structure that presents semantic interfaces, not implementation details. The filesystem exposes the abstraction level relevant to the consumer:

- `tag` — the tag line content (plain text, read/write)
- `body` — the body content at the semantic level (for a shell: command output; for an editor: file content; plain text, read/write)
- `attrs/` — directory of typed attributes, one file per attribute (pane type, title, dirty state, working directory)
- `ctl` — control interface (line commands, write-only)
- `event` — event stream (JSONL, read-only, blocking)

The tree does not expose rendering internals (cell grids, glyph data, buffer state). A script reading `body` gets the content a human would see, at the semantic level the content operates at.

#### Scenario: Tag as plain text
- **WHEN** `cat /srv/pane/1/tag` is executed
- **THEN** the plain text tag line content SHALL be returned without JSON wrapping

#### Scenario: Body as semantic content
- **WHEN** `cat /srv/pane/1/body` is executed
- **THEN** the semantic body content SHALL be returned as plain text (not a cell grid, not rendering state)

#### Scenario: Attributes as individual files
- **WHEN** `cat /srv/pane/1/attrs/title` is executed
- **THEN** the pane's title attribute SHALL be returned as a single value

#### Scenario: Events as JSONL
- **WHEN** `tail -f /srv/pane/1/event` is executed
- **THEN** events SHALL arrive as one JSON object per line

### Requirement: Format per endpoint
Each filesystem node SHALL use the representation natural to its data. Plain text for text data (tag, body). One value per file for attributes (attrs/). Line commands for control files (ctl). JSONL for event streams (event).

#### Scenario: Attribute write
- **WHEN** `echo "new title" > /srv/pane/1/attrs/title` is executed
- **THEN** the pane's title attribute SHALL be updated

### Requirement: Pane index
pane-fs SHALL expose a pane index at `/srv/pane/index`.

#### Scenario: List all panes
- **WHEN** `cat /srv/pane/index` is executed
- **THEN** a listing of all pane IDs SHALL be returned

#### Scenario: Directory listing
- **WHEN** `ls /srv/pane/` is executed
- **THEN** each pane SHALL appear as a directory entry by its ID
