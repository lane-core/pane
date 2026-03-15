## ADDED Requirements

### Requirement: Unified filesystem notification
pane-notify SHALL provide a unified API for filesystem change notification that abstracts over fanotify and inotify. Consumers SHALL request watches by scope and receive events through a calloop-compatible event source.

#### Scenario: Mount-wide watch uses fanotify
- **WHEN** a consumer requests watching an entire mount for attribute changes
- **THEN** pane-notify SHALL use fanotify with FAN_MARK_FILESYSTEM

#### Scenario: Targeted watch uses inotify
- **WHEN** a consumer requests watching a specific directory for file creation/deletion
- **THEN** pane-notify SHALL use inotify

#### Scenario: Calloop integration
- **WHEN** pane-notify is registered as a calloop event source
- **THEN** filesystem events SHALL dispatch in the same event loop iteration as other sources

### Requirement: Watch scope selection
pane-notify SHALL automatically select the appropriate kernel interface based on watch scope. Mount-wide watches SHALL use fanotify. File or directory watches SHALL use inotify. The consumer SHALL not need to specify which interface to use.

#### Scenario: Consumer requests by intent
- **WHEN** a consumer calls `watch_mount("/home", EventKind::Attrib)`
- **THEN** pane-notify SHALL use fanotify internally

- **WHEN** a consumer calls `watch_path("/etc/pane/comp/", EventKind::Create | EventKind::Delete)`
- **THEN** pane-notify SHALL use inotify internally
