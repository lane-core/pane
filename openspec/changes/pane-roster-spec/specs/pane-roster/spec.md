## ADDED Requirements

### Requirement: App ecology
pane-roster is the component that makes the app ecology work. It tracks who's alive, knows what they can do, remembers what was running, and facilitates the protocol flows that launch and connect applications. It does this by implementing the same protocol every other component speaks. It's not special — it's just a server that happens to know about other servers.

The design follows BeOS's BRoster: a directory, not a supervisor. Roster watches. It answers queries. It facilitates launches. It does not restart crashed processes — that's pane-init's job (the abstraction over the host init system). When a process dies and pane-init restarts it, the process re-registers with roster. Roster notices and updates its directory.

**Polarity**: Boundary
**Crate**: `pane-roster`

#### Scenario: Steady state
- **WHEN** the system is running normally
- **THEN** roster knows every server and app, their signatures, capabilities, and connection endpoints

#### Scenario: Server crash and recovery
- **WHEN** pane-route crashes
- **THEN** pane-init restarts it, pane-route re-registers with roster, roster updates its directory — all through the normal protocol, no special crash-handling code

### Requirement: App signatures
Every pane client identifies itself with a reverse-domain signature: `app.pane.shell`, `svc.pane.route`, `app.<vendor>.<name>`. The signature is the identity — used for queries, launch-by-type, single-launch enforcement, session restore, and service discovery.

**Polarity**: Value
**Crate**: `pane-roster`

#### Scenario: Signature as identity
- **WHEN** any component asks "is the browser running?"
- **THEN** it queries roster by signature `app.pane.browser`

### Requirement: Directory queries
Roster answers queries about the running system:
- "Is X running?" — by signature
- "What's running?" — full app list
- "Where is X?" — connection endpoint for a server
- "What can handle this content type?" — service registry query
- "What was I doing?" — recent apps, recent documents

These are the same queries BeOS's BRoster provided. They're how the system discovers itself.

**Crate**: `pane-roster`

#### Scenario: Route needs a handler
- **WHEN** pane-route needs to launch a handler for port "web"
- **THEN** it queries roster for the app registered to handle that port, roster returns the signature and binary path

#### Scenario: Recent items
- **WHEN** a user opens a "recent files" panel
- **THEN** roster provides the recent documents list, each with its associated app signature

### Requirement: Launch facilitation
Roster facilitates app launches — it knows the binary path for a signature and can exec it. But launching is always the result of a protocol flow: something happened (user action, routing rule match, session restore) → a message flowed through the system → roster was asked to start a process.

Roster does not decide *when* to launch. It provides the *how*. The routing rules, the session restorer, the user's click — these decide when.

**Crate**: `pane-roster`

#### Scenario: Launch by content type
- **WHEN** a routing rule matches a JPEG and specifies `start: "app.pane.imageviewer"`
- **THEN** pane-route asks roster to launch app.pane.imageviewer, roster looks up the binary path and execs it

#### Scenario: Single-launch redirect
- **WHEN** a launch request arrives for a signature already running with single-launch enabled
- **THEN** roster activates the existing instance and forwards the message to it

### Requirement: Service registry
Apps register operations they can perform: `(operation, content_type pattern, description)`. This is how the system discovers what's possible — not just what's running, but what it can *do*. pane-route queries this registry to extend routing matches with service capabilities.

When an app disconnects, its registrations are removed. When it reconnects, it re-registers. The protocol handles this — no special lifecycle code.

**Polarity**: Value
**Crate**: `pane-roster`

#### Scenario: Service discovery
- **WHEN** pane-route routes `parse.c:42` and finds both a rule match (editor) and a service match (debugger)
- **THEN** both came from different sources — the rule from a file, the service from roster's registry — but the protocol treats them the same

#### Scenario: Translator rating
- **WHEN** multiple translators can handle a content type
- **THEN** roster provides quality ratings for each, and the best-rated translator is preferred

### Requirement: App database
The app database maps signatures to binary paths, launch arguments, and behavioral flags (single-launch). Stored as files in `~/.config/pane/apps/` (user) and `/etc/pane/apps/` (system). File content is the binary path. Xattrs carry metadata. `ls` to discover all registered apps.

**Crate**: `pane-roster`

#### Scenario: Discoverable
- **WHEN** a user runs `ls ~/.config/pane/apps/`
- **THEN** they see every app signature as a file

#### Scenario: Install an app
- **WHEN** a user creates `~/.config/pane/apps/app.custom.tool` with content `/usr/bin/my-tool`
- **THEN** roster can launch it by signature

### Requirement: Session state
Roster saves and restores the desktop session. Save captures the running app list (signatures, route messages they were launched with). Restore launches them in order. Apps restore their own internal state from their own settings — roster doesn't manage app-internal persistence.

Session state at `~/.config/pane/session/state`.

**Crate**: `pane-roster`

#### Scenario: Session round-trip
- **WHEN** the user has three shell panes and an editor open, saves the session, reboots, and logs in
- **THEN** roster restores three shells and an editor, each with their tag line state and working directories

### Requirement: pane-init contracts
pane-init is the abstraction layer between pane and the host init system. It defines the contracts roster relies on:

1. **Restart guarantee**: if a managed process dies, it will be restarted
2. **Readiness notification**: the init system signals when a process is ready to accept connections
3. **Dependency ordering**: infrastructure servers start in the right order

pane-init maps these contracts to s6 (readiness via `s6-notifyoncheck`), runit (sv check), or systemd (Type=notify, After=). Roster doesn't know which init system is in use. It sees processes appear and register.

**Crate**: `pane-init`

#### Scenario: Init-agnostic
- **WHEN** a pane system runs on systemd vs s6
- **THEN** roster's behavior is identical — processes register the same way regardless of what restarted them

### Requirement: Semantic interface
Roster's interface at `/srv/pane/roster/` presents semantically:

For a **user**:
```
/srv/pane/roster/
  running/          # list running apps (ls to see signatures)
  recent/           # recent apps, recent documents
  launch            # write a signature to launch an app
```

For a **system service**:
```
/srv/pane/roster/
  services/         # registered operations by content type
  endpoints/        # connection endpoints for servers
  session           # save/restore session state
```

#### Scenario: Launch from shell
- **WHEN** a user writes `app.pane.browser` to `/srv/pane/roster/launch`
- **THEN** roster launches the browser
