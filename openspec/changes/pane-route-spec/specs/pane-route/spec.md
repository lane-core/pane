## ADDED Requirements

### Requirement: Routing rule data model
A routing rule SHALL be a JSON file with the following fields:
- `pattern` (string, required): regex pattern to match against the message data
- `port` (string, required): destination port name for matched messages
- `content_type` (string, optional): only match messages with this content type
- `priority` (integer, optional, default 50): lower numbers match first
- `start` (string, optional): app signature to launch if no listener on the port
- `multi` (boolean, optional, default false): if true, collect all matches instead of dispatching first match
- `transform` (object, optional): attrs to add to the routed message (can reference named captures via `$name`)

**Polarity**: Value
**Crate**: `pane-route`

#### Scenario: Simple filename rule
- **WHEN** a rule file contains `{"pattern": "([a-zA-Z_./-]+):(\\d+)", "port": "edit", "transform": {"file": "$1", "addr": "$2"}}`
- **THEN** routing "parse.c:42" SHALL match and deliver to the "edit" port with attrs `file=parse.c` and `addr=42`

#### Scenario: URL rule
- **WHEN** a rule file contains `{"pattern": "https?://[^\\s]+", "port": "web"}`
- **THEN** routing "https://example.com" SHALL match and deliver to the "web" port

#### Scenario: Content type filter
- **WHEN** a rule has `"content_type": "text/rust-error"` and a message has `content_type=text/plain`
- **THEN** the rule SHALL NOT match

### Requirement: Rule file storage
Routing rules SHALL be stored as individual JSON files in well-known directories:
- `~/.config/pane/route/rules/` (user rules)
- `/etc/pane/route/rules/` (system rules)

User rules take precedence over system rules with the same filename. pane-notify SHALL watch these directories for live addition, removal, and modification of rules. Changes SHALL take effect without restarting pane-route.

**Crate**: `pane-route`

#### Scenario: Add rule live
- **WHEN** a new rule file is placed in `~/.config/pane/route/rules/`
- **THEN** pane-route SHALL detect it via pane-notify and add the rule to its active set

#### Scenario: Remove rule live
- **WHEN** a rule file is removed from the rules directory
- **THEN** pane-route SHALL remove the rule from its active set

#### Scenario: User overrides system
- **WHEN** `url.json` exists in both `/etc/pane/route/rules/` and `~/.config/pane/route/rules/`
- **THEN** the user version SHALL take precedence

### Requirement: Matching algorithm
When a RouteCommand message is received, pane-route SHALL evaluate rules in priority order (lowest number first, filename order for ties). For each rule, the regex `pattern` SHALL be tested against the message `data`. The first matching rule dispatches the message to the rule's `port` — unless the rule has `multi: true` or there are registered services that also match.

**Crate**: `pane-route`

#### Scenario: First match wins
- **WHEN** two rules both match the data, rule A has priority 10, rule B has priority 50
- **THEN** rule A SHALL dispatch and rule B SHALL NOT be evaluated

#### Scenario: No match
- **WHEN** no rules match the message data and no services match
- **THEN** the message SHALL be silently dropped (no error)

### Requirement: Named captures as attributes
When a rule's pattern contains named capture groups (`(?P<name>...)`) or numbered groups, matched groups SHALL be added to the routed message's attrs. The `transform` field can reference captures via `$1`, `$2`, or `$name`.

**Crate**: `pane-route`

#### Scenario: Capture groups
- **WHEN** pattern `(?P<file>[^:]+):(?P<line>\\d+)` matches "src/main.rs:42"
- **THEN** the routed message SHALL have attrs `file=src/main.rs` and `line=42`

### Requirement: Port model
pane-route SHALL maintain a set of named ports. Applications register as listeners on ports by connecting to pane-route and sending a listen request. When a message is routed to a port, it SHALL be delivered to all registered listeners. If no listener is registered and the matching rule has a `start` field, pane-route SHALL request pane-roster to launch the specified application.

**Polarity**: Boundary (mediates between Value messages and Compute listeners)
**Crate**: `pane-route`

#### Scenario: Listener receives message
- **WHEN** an editor is listening on port "edit" and a message is routed to "edit"
- **THEN** the editor SHALL receive the RouteMessage

#### Scenario: No listener, start specified
- **WHEN** no application is listening on port "web" and the matching rule has `start: "app.pane.browser"`
- **THEN** pane-route SHALL ask pane-roster to launch `app.pane.browser`, then deliver the message

#### Scenario: No listener, no start
- **WHEN** no application is listening on the target port and no `start` field exists
- **THEN** the message SHALL be silently dropped

### Requirement: Service registry integration
After rule matching, pane-route SHALL query pane-roster's service registry for operations whose `content_type` pattern matches the message data. Matching services are added to the match set alongside rule matches.

**Crate**: `pane-route`

#### Scenario: Rule match + service match
- **WHEN** a rule matches routing "parse.c:42" to port "edit", and a debugger service is registered for content type matching source files
- **THEN** the match set SHALL contain both the rule's port and the service's operation

### Requirement: Multi-match chooser
When the combined match set (rules + services) contains more than one match, pane-route SHALL spawn a transient floating pane (scratchpad) listing all matches as B2-clickable text lines. Each line SHALL show the port/operation name and a brief description. When the user B2-clicks a line, pane-route SHALL dispatch the message to that handler. Plumber rules SHALL take priority (listed first). The chooser pane SHALL dismiss when the user selects an option or when it loses focus.

**Polarity**: Compute (spawns UI, handles user selection)
**Crate**: `pane-route`

#### Scenario: Two matches
- **WHEN** routing "parse.c:42" matches both an editor rule and a debugger service
- **THEN** a chooser pane SHALL appear with lines like:
  ```
  edit  Open in editor at line 42
  debug  Debug parse.c at line 42
  ```

#### Scenario: User selects
- **WHEN** the user B2-clicks "debug" in the chooser
- **THEN** the message SHALL be dispatched to the debugger and the chooser SHALL dismiss

#### Scenario: Single match
- **WHEN** only one rule matches and no services match
- **THEN** the message SHALL auto-dispatch without showing a chooser
