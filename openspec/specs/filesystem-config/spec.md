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
