## ADDED Requirements

### Requirement: App signatures
Every pane client SHALL identify itself with a reverse-domain signature string (e.g., `app.pane.shell`, `svc.pane.route`). Infrastructure servers use the `svc.pane.*` prefix. Desktop applications use `app.pane.*` or `app.<vendor>.<name>`. The signature is sent at registration time and used for queries, single-launch enforcement, and session restore.

**Polarity**: Value
**Crate**: `pane-roster`

#### Scenario: Registration with signature
- **WHEN** pane-shell connects to pane-roster
- **THEN** it SHALL send a RosterRegister message with `signature: "app.pane.shell"` and `kind: Application`

#### Scenario: Infrastructure registration
- **WHEN** pane-route starts and connects to pane-roster
- **THEN** it SHALL send a RosterRegister message with `signature: "svc.pane.route"` and `kind: Infrastructure`

### Requirement: Service directory for infrastructure
pane-roster SHALL accept RosterRegister messages from infrastructure servers and maintain a directory of their identities, capabilities, and connection endpoints. Roster SHALL NOT supervise infrastructure servers — the init system does that. When an infrastructure server disconnects (crash or shutdown), roster SHALL remove it from the directory. When it reconnects (after init restarts it), roster SHALL re-add it.

**Polarity**: Boundary
**Crate**: `pane-roster`

#### Scenario: Infrastructure query
- **WHEN** a client queries "where is the router?"
- **THEN** roster SHALL return the socket path for `svc.pane.route` if registered, or indicate it's not running

#### Scenario: Infrastructure crash and restart
- **WHEN** pane-route crashes and the init system restarts it
- **THEN** pane-route SHALL re-register with roster, and roster SHALL update its directory

### Requirement: Process supervisor for desktop apps
pane-roster SHALL launch, monitor, and optionally restart desktop applications. Desktop apps are processes that connect via the pane protocol and create panes. Roster tracks their PID, signature, pane IDs, and exit status.

**Polarity**: Compute (active supervision)
**Crate**: `pane-roster`

#### Scenario: Launch request
- **WHEN** pane-route requests launching `app.pane.browser` (no listener on "web" port)
- **THEN** roster SHALL fork/exec the browser, track its PID, and report success

#### Scenario: App exit tracking
- **WHEN** a supervised desktop app exits
- **THEN** roster SHALL detect the exit via SIGCHLD/waitpid and update its state

### Requirement: Restart policy
Desktop apps SHALL have a configurable restart policy per signature: `restart-on-crash` (default), `restart-always`, or `restart-never`. Crash is defined as exit by signal or non-zero exit code. Clean exit is zero exit code or closure via pane protocol CloseRequested.

Restart uses exponential backoff: 1s, 2s, 4s, 8s, up to 60s maximum. After 5 consecutive crashes without the app surviving for at least 10 seconds, roster SHALL stop restarting and log an error.

**Hazard**: A misconfigured app that crashes immediately can consume resources during the backoff period.

**Crate**: `pane-roster`

#### Scenario: Crash restart
- **WHEN** a desktop app exits with SIGSEGV
- **THEN** roster SHALL restart it after the current backoff delay

#### Scenario: Clean exit
- **WHEN** a desktop app exits with code 0
- **THEN** roster SHALL NOT restart it (under `restart-on-crash` policy)

#### Scenario: Restart backoff exhaustion
- **WHEN** an app crashes 5 times within 50 seconds
- **THEN** roster SHALL stop restarting and log "app.pane.foo: restart backoff exhausted"

### Requirement: Single-launch enforcement
Apps MAY declare single-launch behavior. When a launch request arrives for a signature that is already running with single-launch enabled, roster SHALL NOT launch a new instance. Instead, roster SHALL activate (bring to front) the existing instance and forward the route message to it.

**Crate**: `pane-roster`

#### Scenario: Single-launch redirect
- **WHEN** a launch request for `app.pane.browser` arrives and an instance is already running with single-launch
- **THEN** roster SHALL forward the route message to the existing instance instead of launching a new one

#### Scenario: Multi-instance allowed
- **WHEN** a launch request for `app.pane.shell` arrives (no single-launch)
- **THEN** roster SHALL launch a new instance regardless of existing instances

### Requirement: Service registry
Desktop apps SHALL be able to register operations they can perform on content types. Each registration includes: `operation` (name), `content_type` (glob pattern), and `description` (human-readable). pane-route queries this registry when matching route messages.

**Polarity**: Value (registrations are data)
**Crate**: `pane-roster`

#### Scenario: Service registration
- **WHEN** an editor registers `{operation: "format-json", content_type: "application/json", description: "Format JSON content"}`
- **THEN** roster SHALL store this registration and make it queryable

#### Scenario: Service query
- **WHEN** pane-route queries "what operations match content_type text/x-rust?"
- **THEN** roster SHALL return all registrations whose content_type glob matches `text/x-rust`

#### Scenario: Service deregistration on disconnect
- **WHEN** the editor disconnects
- **THEN** roster SHALL remove all its service registrations

### Requirement: App database
pane-roster SHALL maintain an app database mapping signatures to binary paths, launch arguments, restart policies, and single-launch flags. The database SHALL be stored as files in `~/.config/pane/apps/` (one file per signature) with system defaults at `/etc/pane/apps/`. The format follows filesystem-as-configuration: file content is the binary path, xattrs carry metadata.

**Crate**: `pane-roster`

#### Scenario: Launch by signature
- **WHEN** roster needs to launch `app.pane.browser`
- **THEN** it SHALL look up `~/.config/pane/apps/app.pane.browser` (or `/etc/pane/apps/app.pane.browser`), read the binary path from file content, and exec it

#### Scenario: Discoverable apps
- **WHEN** a user runs `ls ~/.config/pane/apps/`
- **THEN** all registered app signatures SHALL be listed as files

### Requirement: Session save and restore
pane-roster SHALL support saving and restoring the desktop session. Save captures: the list of running desktop apps (signatures + any route messages they were launched with). Restore launches the apps in the saved order. Apps restore their own internal state from their own settings.

Session state SHALL be stored at `~/.config/pane/session/state` using pane-proto serialization (postcard).

**Crate**: `pane-roster`

#### Scenario: Session save
- **WHEN** the user initiates session save (or the compositor shuts down cleanly)
- **THEN** roster SHALL serialize the running app list to `~/.config/pane/session/state`

#### Scenario: Session restore
- **WHEN** pane-roster starts and finds `~/.config/pane/session/state`
- **THEN** roster SHALL launch the saved apps in order

#### Scenario: App not found on restore
- **WHEN** a saved app signature has no entry in the app database
- **THEN** roster SHALL log a warning and skip that app
