## ADDED Requirements

### Requirement: Communication infrastructure
pane-route is pane's communication infrastructure — the kit and server that handles how data flows between processes, users, and the system. It unifies local text routing (B3-click → handler), inter-process messaging (server ↔ server), and protocol bridging (D-Bus, 9P, network) under a single model: data arrives, is matched and transformed by rules, and dispatches to a handler.

pane-route is strictly complementary to Linux's existing infrastructure. It does not replace sockets, D-Bus, or networking. It provides typed abstractions over them for pane applications, and bridges between foreign protocols and the native pane message model.

**Polarity**: Boundary (mediates all data flow)
**Crate**: `pane-route` (server), `pane-route-lib` (kit)

#### Scenario: Text routing
- **WHEN** a user B3-clicks `src/main.rs:42`
- **THEN** pane-route matches the text, extracts file and line, and dispatches to the editor

#### Scenario: D-Bus bridge
- **WHEN** BlueZ sends a D-Bus signal that a device connected
- **THEN** pane-dbus translates it to a pane route message, which matches a notification rule and appears in a notification pane

#### Scenario: Network request
- **WHEN** a pane application wants to fetch a URL
- **THEN** it sends a route message with the URL; a rule matches and dispatches to the appropriate handler (browser, downloader, or inline preview)

### Requirement: Routing rules
A routing rule is a declarative specification of how content matches and where it goes. Rules are files in well-known directories, one per file, watched by pane-notify for live modification.

Rule fields:
- `pattern` (string, required): regex to match against message data
- `port` (string, required): destination port for matched messages
- `content_type` (string, optional): match only messages of this type
- `priority` (integer, optional, default 50): lower = higher priority
- `start` (string, optional): app signature to launch if no listener
- `multi` (boolean, optional, default false): collect all matches for chooser
- `transform` (object, optional): attrs to add, referencing captures via `$name`

Rule storage:
- `~/.config/pane/route/rules/` (user, takes precedence)
- `/etc/pane/route/rules/` (system)

**Polarity**: Value
**Crate**: `pane-route`

#### Scenario: File path rule
- **WHEN** a rule matches `(?P<file>[^:]+):(?P<line>\d+)` against `parse.c:42`
- **THEN** the message dispatches to port "edit" with attrs `file=parse.c`, `line=42`

#### Scenario: Live rule addition
- **WHEN** a new rule file appears in the rules directory
- **THEN** pane-route detects it via pane-notify and adds it immediately

### Requirement: Port model
Ports are named endpoints where handlers listen. Applications register interest in ports. Messages routed to a port are delivered to all listeners. If no listener exists and the rule specifies `start`, pane-roster launches the handler app.

**Polarity**: Boundary
**Crate**: `pane-route`

#### Scenario: Listener receives
- **WHEN** an editor listens on port "edit" and a message routes to "edit"
- **THEN** the editor receives the message

#### Scenario: Auto-launch
- **WHEN** no listener on port "web" and the rule has `start: "app.pane.browser"`
- **THEN** roster launches the browser, then the message is delivered

### Requirement: Service registry integration
After rule matching, pane-route queries pane-roster for registered service operations that match the content. Services extend the match set. Multiple matches → transient chooser pane with B2-clickable options. Single match auto-dispatches.

**Crate**: `pane-route`

#### Scenario: Multi-match chooser
- **WHEN** both an editor rule and a debugger service match `parse.c:42`
- **THEN** a chooser pane lists both; user B2-clicks to select

### Requirement: Protocol bridges
Foreign protocols are integrated via bridge services — small daemons that translate between a foreign protocol and pane's native message model. Each bridge is a separate process, registered with pane-roster, translating bidirectionally.

The bridge pattern:
- **pane-dbus**: translates D-Bus signals/method calls ↔ pane route messages. System events (hardware, network, power) become routable content. Pane apps can call D-Bus services through pane's typed interface.
- **pane-9p**: serves pane state as a 9P filesystem. Remote Plan 9 systems (or plan9port tools) can interact with pane natively.
- **Additional bridges** follow the same pattern: speak the foreign protocol on one side, emit/consume pane messages on the other. The pane side is always the same typed interface.

Bridges are plugins — drop the binary in a directory, register with roster, add routing rules for the messages it produces. The user doesn't think about D-Bus; they see the result of a D-Bus signal matched by a routing rule.

**Crate**: `pane-dbus`, `pane-9p`, etc. (each is its own crate)

#### Scenario: D-Bus notification
- **WHEN** NetworkManager signals a wifi connection via D-Bus
- **THEN** pane-dbus translates it to a route message with `content_type: "system/network-event"`, a routing rule matches, and a notification appears

#### Scenario: 9P remote access
- **WHEN** a remote system mounts pane's 9P interface
- **THEN** it can read pane state (pane list, tag lines, body content) and write commands (create panes, execute text) using standard 9P operations

### Requirement: Transport abstraction
pane-route-lib SHALL provide a transport abstraction for pane applications. Applications describe what they want to communicate (content, destination, semantics), not how (socket type, protocol, serialization). The kit selects the appropriate transport: Unix sockets for local, TCP for remote, file operations for filesystem.

This is the network kit aspect — complementary to Linux networking, providing typed pane-native wrappers without replacing the underlying infrastructure.

**Polarity**: Boundary
**Crate**: `pane-route-lib`

#### Scenario: Local message
- **WHEN** a pane app sends a route message to a local handler
- **THEN** pane-route-lib uses a Unix socket

#### Scenario: Remote message
- **WHEN** a pane app sends a message to a handler on a remote pane instance
- **THEN** pane-route-lib uses TCP with the pane protocol

### Requirement: Semantic interface
pane-route's own interface at `/srv/pane/route/` presents semantically:

For a **user** (scripting):
```
/srv/pane/route/
  send            # write text to route it (plain text or JSON)
  rules/          # list/read/write routing rules
  ports/          # list active ports and their listeners
```

For a **system service** (programmatic):
```
/srv/pane/route/
  match           # write text, read back matching rules/services (query without dispatch)
  stats           # routing statistics (matches, drops, bridge activity)
```

#### Scenario: Route from shell
- **WHEN** a user writes `echo "parse.c:42" > /srv/pane/route/send`
- **THEN** the text is routed as if B3-clicked

#### Scenario: Query matches
- **WHEN** a system service writes to `/srv/pane/route/match`
- **THEN** it receives back the list of rules and services that would match, without dispatching
