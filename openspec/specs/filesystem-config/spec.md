## ADDED Requirements

### Requirement: Configuration as files
Server configuration SHALL be stored as files in well-known directories under `/etc/pane/<server>/`. Each config key SHALL be a separate file. The file content SHALL be the value. xattrs on the file SHALL carry metadata about the config entry.

#### Scenario: Config value is file content
- **WHEN** `cat /etc/pane/comp/font` is executed
- **THEN** the output SHALL be the configured font name (e.g., "Iosevka")

#### Scenario: Config change via write
- **WHEN** `echo "JetBrains Mono" > /etc/pane/comp/font` is executed
- **THEN** the compositor SHALL detect the change via pane-notify and apply the new font

### Requirement: Config metadata in xattrs
Config files SHALL carry xattrs describing the entry: `user.pane.type` (string, int, float, bool), `user.pane.description` (human-readable description). Optional xattrs include `user.pane.range` (valid range for numeric values), `user.pane.options` (valid options for enum values).

#### Scenario: Type metadata
- **WHEN** `getfattr -n user.pane.type /etc/pane/comp/font-size` is executed
- **THEN** the output SHALL indicate the type is "int"

#### Scenario: Config tooling
- **WHEN** a config tool reads `/etc/pane/comp/` and its xattrs
- **THEN** it SHALL have enough information to present appropriate input controls (text field for strings, slider for bounded ints, etc.)

### Requirement: Reactive configuration
Servers SHALL watch their config directories via pane-notify. Config changes SHALL take effect without server restart, without SIGHUP, and without manual reload commands.

#### Scenario: Live font change
- **WHEN** the font config file is modified while the compositor is running
- **THEN** the compositor SHALL re-render all panes with the new font on the next frame

### Requirement: Discoverable configuration
All available config keys for a server SHALL be discoverable by listing its config directory. Every valid config key SHALL have a corresponding file, even if set to the default value.

#### Scenario: List all compositor config
- **WHEN** `ls /etc/pane/comp/` is executed
- **THEN** all compositor config keys SHALL be listed as files
