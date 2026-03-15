## MODIFIED Requirements

### Requirement: Inter-server protocol
Inter-server communication SHALL use `PaneMessage<ServerVerb>` with typed views for field access. Raw attr key access (`msg.attr("key")`) SHALL NOT be used in production code paths — only inside typed view `parse()` implementations. Each server SHALL define its own typed views and builders in per-server modules within pane-proto.

**Polarity**: Boundary
**Crate**: `pane-proto::server`

#### Scenario: Typed view enforces discipline
- **WHEN** server dispatch logic accesses inter-server message fields
- **THEN** it SHALL do so through a typed view's accessor methods, not raw attr access
