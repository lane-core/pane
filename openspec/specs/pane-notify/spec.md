## ADDED Requirements

### Requirement: Unified filesystem notification
pane-notify SHALL provide a unified API for filesystem change notification that abstracts over fanotify and inotify. Consumers SHALL request watches by scope and receive events through the consumer's message queue — as messages in the looper for looper-based servers, or through a channel for channel-based consumers. The compositor is the sole exception: it receives events via a calloop event source, because calloop is scoped to the compositor (architecture spec §3, §6).

#### Scenario: Mount-wide watch uses fanotify
- **WHEN** a consumer requests watching an entire mount for attribute changes
- **THEN** pane-notify SHALL use fanotify with FAN_MARK_FILESYSTEM and FAN_ATTRIB
- **AND** events SHALL be delivered as messages to the consumer's looper or channel

#### Scenario: Targeted watch uses inotify
- **WHEN** a consumer requests watching a specific directory for file creation/deletion
- **THEN** pane-notify SHALL use inotify

#### Scenario: Looper integration
- **WHEN** a looper-based server (pane-store, pane-roster, or a pane-app client) registers a pane-notify watch
- **THEN** filesystem events SHALL be delivered as typed messages to the server's message queue
- **AND** events SHALL be processed sequentially within the looper, like any other message

#### Scenario: Compositor integration
- **WHEN** pane-comp registers a pane-notify watch
- **THEN** pane-notify SHALL provide a calloop-compatible event source
- **AND** filesystem events SHALL dispatch in the same event loop iteration as other sources
- **NOTE** This is the only consumer that uses calloop. calloop does not define the system-wide concurrency model.

### Requirement: Watch scope selection
pane-notify SHALL automatically select the appropriate kernel interface based on watch scope. Mount-wide watches SHALL use fanotify. File or directory watches SHALL use inotify. The consumer SHALL not need to specify which interface to use.

#### Scenario: Consumer requests by intent
- **WHEN** a consumer calls `watch_mount("/home", EventKind::Attrib)`
- **THEN** pane-notify SHALL use fanotify internally

- **WHEN** a consumer calls `watch_path("/etc/pane/comp/", EventKind::Create | EventKind::Delete)`
- **THEN** pane-notify SHALL use inotify internally

### Requirement: FAN_ATTRIB for xattr change detection
pane-notify SHALL use fanotify FAN_ATTRIB events for detecting xattr changes. The VFS layer emits FS_ATTRIB (mapped to FAN_ATTRIB) after every setxattr() and removexattr() call, uniformly across all filesystems. This is the mechanism that makes pane-store's mount-wide attribute indexing possible without recursive directory walking.

#### Scenario: xattr change detected
- **WHEN** any process calls setxattr() or removexattr() on a file within a watched filesystem
- **THEN** pane-notify SHALL receive a FAN_ATTRIB event with the file's handle (via FAN_REPORT_FID)
- **AND** pane-notify SHALL deliver an Attrib event to the consumer

#### Scenario: Consumer must disambiguate
- **GIVEN** FAN_ATTRIB does not distinguish which attribute changed or what kind of metadata change occurred (xattr, chmod, chown, utimes all produce the same event)
- **WHEN** a consumer receives an Attrib event
- **THEN** the consumer is responsible for re-reading the relevant attributes and diffing against cached values
- **NOTE** pane-store's event handler reads user.pane.* xattrs on each Attrib event and updates its index only if pane-relevant attributes changed

### Requirement: Capability awareness
fanotify with FAN_MARK_FILESYSTEM requires CAP_SYS_ADMIN. pane-notify SHALL document this requirement. Only privileged consumers (pane-store, running as a system service) can request mount-wide watches. Unprivileged consumers are limited to inotify-based targeted watches.

#### Scenario: Unprivileged consumer requests mount-wide watch
- **WHEN** an unprivileged consumer calls `watch_mount()`
- **THEN** pane-notify SHALL return an error indicating insufficient capabilities
- **AND** the error message SHALL suggest using `watch_path()` for targeted watching
