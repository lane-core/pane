## ADDED Requirements

### Requirement: FUSE service at /srv/pane/
pane-fs SHALL expose compositor, plumber, and configuration state as a FUSE filesystem mounted at `/srv/pane/`. pane-fs SHALL be a separate server process that communicates with other pane servers via the socket protocol.

#### Scenario: Mount point available
- **WHEN** pane-fs starts
- **THEN** `/srv/pane/` SHALL be accessible as a filesystem

#### Scenario: Separate process
- **WHEN** pane-comp crashes and restarts
- **THEN** pane-fs SHALL reconnect and continue serving the filesystem

### Requirement: Format per endpoint
Each filesystem node SHALL use the representation natural to its data. Plain text for text data (tag, body). JSON for structured data (cells, attrs, index). Line commands for control files (ctl). JSONL for event streams (event, plumb ports).

#### Scenario: Tag as plain text
- **WHEN** `cat /srv/pane/1/tag` is executed
- **THEN** the plain text tag line content SHALL be returned without JSON wrapping

#### Scenario: Cells as JSON
- **WHEN** `cat /srv/pane/1/cells` is executed
- **THEN** full cell grid data SHALL be returned as JSON with character, color, and attribute fields

#### Scenario: Events as JSONL
- **WHEN** `tail -f /srv/pane/1/event` is executed
- **THEN** events SHALL arrive as one JSON object per line

### Requirement: Plumber filesystem interface
pane-fs SHALL expose plumber ports under `/srv/pane/plumb/`. Writing to `send` SHALL route a plumb message. Reading from a named port SHALL stream matched messages as JSONL.

#### Scenario: Plumb from shell
- **WHEN** a user writes a plumb message to `/srv/pane/plumb/send`
- **THEN** the plumber SHALL receive and route it

### Requirement: Configuration filesystem interface
pane-fs SHALL expose server configuration under `/srv/pane/config/` mirroring the structure of `/etc/pane/`. Reading a config file SHALL return the current value. Writing SHALL update the value and trigger the relevant server to reload via pane-notify.

#### Scenario: Read config via FUSE
- **WHEN** `cat /srv/pane/config/comp/font` is executed
- **THEN** the current compositor font name SHALL be returned

#### Scenario: Write config via FUSE
- **WHEN** `echo "JetBrains Mono" > /srv/pane/config/comp/font` is executed
- **THEN** the compositor SHALL pick up the font change
