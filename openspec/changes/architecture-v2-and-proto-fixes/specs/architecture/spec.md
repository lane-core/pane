## MODIFIED Requirements

### Requirement: Target platform
Pane SHALL target Linux exclusively, tracking the latest stable kernel release. The system SHALL NOT target macOS, BSD, or other Unix-like operating systems. The system SHALL leverage Linux-specific capabilities: mount namespaces, user namespaces, fanotify, inotify, xattrs, memfd, pidfd, and seccomp.

#### Scenario: Linux-only build
- **WHEN** pane is compiled
- **THEN** it SHALL compile only on Linux targets

### Requirement: Init system
Pane infrastructure servers SHALL be managed by a supervision-tree init system (s6 or runit). Desktop applications SHALL be managed by pane-roster. The system SHALL NOT depend on systemd.

#### Scenario: Infrastructure server restart
- **WHEN** pane-plumb crashes
- **THEN** the init system's supervisor SHALL restart it, and pane-plumb SHALL re-register with pane-roster

### Requirement: Filesystem requirements
The target filesystem SHALL support the `user.*` xattr namespace. ext4, btrfs, XFS, and bcachefs all qualify. Advanced filesystem features (snapshots, subvolumes, CoW) SHALL be available through an abstraction layer when the filesystem provides them, and SHALL degrade gracefully on filesystems that lack them.

#### Scenario: xattr support
- **WHEN** pane-store writes a `user.pane.*` xattr
- **THEN** the filesystem SHALL persist it and make it readable

#### Scenario: Graceful degradation
- **WHEN** a snapshot operation is requested on ext4
- **THEN** the system SHALL report that snapshots are not available on this filesystem, without error

### Requirement: Design pillars — filesystem as interface
The architecture SHALL include "Filesystem as Interface" as a design principle. Configuration SHALL be files in directories with xattrs for metadata. Plugin discovery SHALL be via well-known directories watched by pane-notify. The FUSE interface at `/srv/pane/` SHALL expose server state as a filesystem. The filesystem IS the database, the registry, and the configuration format.

#### Scenario: Configuration via filesystem
- **WHEN** an administrator writes a new value to a config file under `/etc/pane/`
- **THEN** the relevant server SHALL detect the change via pane-notify and apply it without restart

#### Scenario: Plugin discovery via filesystem
- **WHEN** a translator shared library is placed in `~/.config/pane/translators/`
- **THEN** the relevant server SHALL detect it via pane-notify and load the new capability

### Requirement: Servers — pane-fs
The system SHALL include a pane-fs server that exposes compositor and plumber state as a FUSE filesystem at `/srv/pane/`. pane-fs SHALL be a separate process that speaks the pane socket protocol to other servers. pane-fs SHALL use format-per-endpoint: plain text for text data, JSON for structured data, line commands for control files, JSONL for event streams.

#### Scenario: Read pane body
- **WHEN** a user runs `cat /srv/pane/1/body`
- **THEN** the plain text content of pane 1's cell grid SHALL be returned

#### Scenario: Write pane control
- **WHEN** a user runs `echo close > /srv/pane/1/ctl`
- **THEN** pane 1 SHALL receive a close request

### Requirement: Servers — pane-notify
The system SHALL include a pane-notify module or crate that abstracts over Linux filesystem notification interfaces. pane-notify SHALL use fanotify for mount-wide watches and inotify for targeted watches. Consumers SHALL request watches by scope and receive a unified event stream.

#### Scenario: Mount-wide xattr watch
- **WHEN** pane-store requests watching a mount for xattr changes
- **THEN** pane-notify SHALL use fanotify with FAN_MARK_FILESYSTEM and FAN_ATTRIB

#### Scenario: Targeted directory watch
- **WHEN** a live query watches a specific directory
- **THEN** pane-notify SHALL use inotify for that directory

### Requirement: Roster hybrid model
pane-roster SHALL act as a service directory for infrastructure servers (which register on startup) and as a process supervisor for desktop applications. Infrastructure servers SHALL be supervised by the init system. Desktop applications SHALL be launched, monitored, and restarted by pane-roster.

#### Scenario: Infrastructure server registration
- **WHEN** pane-plumb starts and connects to pane-roster
- **THEN** pane-roster SHALL record its identity and capabilities without assuming supervision responsibility

#### Scenario: Desktop app crash restart
- **WHEN** a shell pane client crashes
- **THEN** pane-roster SHALL restart it and restore its pane

### Requirement: Plumber multi-match
When multiple handlers match a plumb message (plumber rules and registered services combined), the plumber SHALL spawn a transient floating pane listing the options as B2-clickable text. When exactly one handler matches, the plumber SHALL auto-dispatch. Plumber rules SHALL take priority over registered services.

#### Scenario: Single match auto-dispatch
- **WHEN** B3-clicking `parse.c:42` and only the editor rule matches
- **THEN** the editor SHALL open parse.c at line 42 without presenting a chooser

#### Scenario: Multiple match chooser
- **WHEN** B3-clicking `parse.c:42` and both an editor rule and a debugger service match
- **THEN** a transient floating pane SHALL appear listing both options as clickable text
