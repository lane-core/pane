## ADDED Requirements

### Requirement: Dual license split
The pane project SHALL use a dual-license model. Kit crates (libraries linked into client processes) SHALL be MIT licensed. Server crates (infrastructure processes) SHALL be AGPL-3.0-only licensed.

**MIT crates (kits and protocol):** pane-proto, pane-session, pane-app, pane-ui, pane-text, pane-input, pane-store-client, pane-media, pane-ai, pane-notify
**AGPL-3.0-only crates (servers):** pane-comp, pane-roster, pane-store, pane-fs, pane-watchdog, pane-dbus

**Rationale:** MIT on kits and protocol maximizes adoption — anyone can build pane clients, and the kits ARE the developer experience. AGPL on servers ensures modifications to infrastructure are shared, including in distributed computing scenarios where the network clause applies.

**The boundary:** A kit is a library that lives inside the client process. It cannot crash independently of the application that uses it. A server is a separate process that provides infrastructure services over unix sockets. This distinction — in-process library vs. independent service — is the license boundary.

#### Scenario: Proprietary pane client
- **WHEN** a third party builds a proprietary application using pane kits (pane-proto, pane-app, pane-ui, etc.)
- **THEN** the MIT license SHALL permit this without requiring source disclosure

#### Scenario: Modified compositor shared
- **WHEN** someone modifies pane-comp and deploys it (locally or as a network service)
- **THEN** the AGPL-3.0 license SHALL require sharing the modified source

#### Scenario: Custom agent kit usage
- **WHEN** a third party uses pane-ai to build proprietary agent infrastructure
- **THEN** the MIT license SHALL permit this — the kit is a library, not a service
