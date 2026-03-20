## ADDED Requirements

### Requirement: Dual license split
The pane project SHALL use a dual-license model. Protocol crates and client kits SHALL be MIT licensed. Server implementations SHALL be AGPL-3.0-only licensed.

**MIT crates:** pane-proto, pane-app, pane-ui, pane-text, pane-store-client, pane-notify
**AGPL-3.0-only crates:** pane-comp, pane-roster, pane-store, pane-fs

**Rationale:** MIT on the protocol maximizes adoption — anyone can build pane clients. AGPL on servers ensures modifications to infrastructure are shared, including in distributed computing scenarios where the network clause applies.

#### Scenario: Proprietary pane client
- **WHEN** a third party builds a proprietary application using pane-proto
- **THEN** the MIT license SHALL permit this without requiring source disclosure

#### Scenario: Modified compositor shared
- **WHEN** someone modifies pane-comp and deploys it (locally or as a network service)
- **THEN** the AGPL-3.0 license SHALL require sharing the modified source
