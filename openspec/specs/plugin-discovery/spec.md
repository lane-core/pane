## ADDED Requirements

### Requirement: Filesystem-based plugin registration
Servers that support extensibility SHALL discover plugins by scanning well-known directories. Adding a file to the directory SHALL register the plugin. Removing it SHALL unregister it. pane-notify SHALL watch these directories for live discovery.

#### Scenario: Add translator
- **WHEN** a translator binary is placed in `~/.config/pane/translators/`
- **THEN** the system SHALL detect it and make the new content type available without restart

#### Scenario: Remove translator
- **WHEN** a translator binary is removed from `~/.config/pane/translators/`
- **THEN** the system SHALL stop offering that content type capability

### Requirement: Well-known plugin directories
The system SHALL define well-known directories for each plugin category:
- `~/.config/pane/translators/` — content translators (type sniffing, format conversion)
- `~/.config/pane/input/` — input method add-ons
- `~/.config/pane/plumb/rules/` — plumber rules (one file per rule)

System-wide equivalents SHALL exist under `/etc/pane/` with user directories taking precedence.

#### Scenario: User overrides system
- **WHEN** a translator exists in both `/etc/pane/translators/` and `~/.config/pane/translators/` with the same name
- **THEN** the user's version SHALL take precedence

### Requirement: Plugin metadata
Plugins SHALL carry metadata in xattrs describing their capabilities: `user.pane.plugin.type` (translator, input-method, rule), `user.pane.plugin.handles` (content types or patterns the plugin handles), `user.pane.plugin.description` (human-readable description).

#### Scenario: Plugin introspection
- **WHEN** `getfattr -d ~/.config/pane/translators/webp` is executed
- **THEN** xattrs SHALL indicate what content types the translator handles
