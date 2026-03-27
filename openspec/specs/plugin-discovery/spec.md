## ADDED Requirements

### Requirement: Filesystem-based plugin registration
Components that support extensibility SHALL discover plugins by scanning well-known directories. Adding a file or directory to the well-known location SHALL register the plugin. Removing it SHALL unregister it. The pane-app kit uses pane-notify (fanotify/inotify abstraction) to watch these directories for live discovery — this is a kit-level concern, not a server-level one. Each process that needs plugin awareness loads and watches its own relevant directories. Installed plugins and their metadata are visible through pane-fs at `/srv/pane/` alongside pane state.

#### Scenario: Add translator
- **WHEN** a translator binary is placed in `~/.config/pane/translators/`
- **THEN** any process using the pane-app kit SHALL detect it via pane-notify and make the new content type available without restart

#### Scenario: Remove translator
- **WHEN** a translator binary is removed from `~/.config/pane/translators/`
- **THEN** any process using the pane-app kit SHALL stop offering that content type capability

### Requirement: Well-known plugin directories
The system SHALL define well-known directories for each plugin category:
- `~/.config/pane/translators/` — content translators following the Translation Kit pattern (type sniffing, format conversion, quality-rated multi-handler selection)
- `~/.config/pane/route/rules/` — routing rules (one file per rule, loaded by the pane-app kit for local evaluation)
- `~/.config/pane/input/` — input method add-ons
- `~/.config/pane/apps/` — `.app` directories (application bundles containing binary/wrapper, integration metadata, pane-specific hooks, routing rules, `.plan`-governed agent companions)

Agent `.plan` files are not plugins — they live in each agent's home directory (`~agent/.plan`) as the agent's identity and behavior specification. Discovery of active agents is a pane-roster concern, not a plugin directory concern.

System-wide equivalents SHALL exist under `/etc/pane/` with user directories taking precedence.

#### Scenario: User overrides system translator
- **WHEN** a translator exists in both `/etc/pane/translators/` and `~/.config/pane/translators/` with the same name
- **THEN** the user's version SHALL take precedence

#### Scenario: User adds routing rule
- **WHEN** a rule file is dropped into `~/.config/pane/route/rules/`
- **THEN** the pane-app kit SHALL detect it via pane-notify and begin evaluating it on subsequent route actions — no restart required

#### Scenario: Install a .app directory
- **WHEN** a `.app` directory is placed in `~/.config/pane/apps/`
- **THEN** pane-roster SHALL detect it via pane-notify, register the application's metadata (launch semantics, content types, quality ratings), and make it launchable. Roster owns `.app` discovery because app lifecycle and launch semantics are roster concerns.

### Requirement: Translation Kit pattern for translators
Translators SHALL follow the Translation Kit pattern from the architecture spec (§4 pane-app, §8 Composition Model). Each translator declares:
- The content types it can read (input formats)
- The content types it can produce (output formats)
- A self-declared quality rating per conversion path

When multiple translators match a content type, the pane-app kit selects by quality rating. This enables automatic best-handler selection without central authority. The number of translators scales linearly (one per format), not quadratically (one per format pair).

#### Scenario: Multiple translators for same type
- **WHEN** two translators both handle `image/webp` with quality ratings 0.8 and 0.95
- **THEN** the pane-app kit SHALL select the 0.95-rated translator by default

#### Scenario: Drop-in format support
- **WHEN** a translator for a new format (e.g., AVIF) is dropped into the translators directory
- **THEN** the entire system gains that format capability — file managers display thumbnails, routing rules can dispatch to it, agents can process it

### Requirement: Plugin metadata via xattrs
Plugins SHALL carry metadata in xattrs on the btrfs filesystem (the architecture spec commits to btrfs exclusively — ext4's ~4KB xattr limit is insufficient; btrfs provides ~16KB per value with no per-inode total limit). Metadata attributes:
- `user.pane.plugin.type` — plugin category (translator, input-method, rule, app)
- `user.pane.plugin.handles` — content types or patterns the plugin handles
- `user.pane.plugin.quality` — self-declared quality rating (translators)
- `user.pane.plugin.description` — human-readable description

These attributes are indexed by pane-store, making plugins queryable: "which translators handle image/*?" is a pane-store query, not a directory scan.

#### Scenario: Plugin introspection
- **WHEN** `getfattr -d ~/.config/pane/translators/webp` is executed
- **THEN** xattrs SHALL indicate what content types the translator handles, its quality rating, and its description

#### Scenario: Query available translators
- **WHEN** a component queries pane-store for `user.pane.plugin.type == "translator" && user.pane.plugin.handles == "image/*"`
- **THEN** pane-store SHALL return all matching translator files from both system and user directories
