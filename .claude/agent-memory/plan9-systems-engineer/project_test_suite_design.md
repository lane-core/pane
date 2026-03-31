---
name: Test suite design for distributed pane
description: Three-layer test taxonomy (protocol/transport/location-transparency) and MockCompositor vs headless roles — decided 2026-03-30
type: project
---

Test suite restructuring decisions for distributed pane operation:

1. **Three test layers, not transport-parameterized everything.**
   - Protocol semantics: MemoryTransport only, tests the state machine
   - Transport integration: one test per transport (unix, tcp), tests framing/connection
   - Location transparency: same scenario run against MockCompositor and TestHeadless, small focused set (~5-10 scenarios)

2. **MockCompositor stays dumb, headless becomes integration server.**
   - MockCompositor = dumb protocol echo for unit-testing kit behavior. Make handshake handler pluggable (closure receiving ClientHello, returns accept/reject)
   - TestHeadless = pane-headless in test mode, the real server for integration tests. Build when pane-headless exists
   - Don't make MockCompositor smart — keep it predictable, test correctness against real server

3. **Identity flow: test the decision contract, not serialization.**
   - Test that PeerIdentity reaches server decision point and server decision reaches client
   - Two-three tests: identity present (Remote), identity None (Local), rejection case

4. **PaneId test discipline: create once, reuse.**
   - `pane_id(n)` returning random UUID is correct. Tests must store the ID and reuse it
   - Current handler tests have a latent bug: `pane_id(1)` called separately for make_handle and send_comp creates two different IDs. Works by accident because run_handler doesn't filter by PaneId yet

**Why:** Lane asked for Plan 9-informed test design principles after UUID PaneId and network transparency extensions broke assumptions in the test suite.

**How to apply:** Reference when restructuring pane-app tests and when building pane-headless test infrastructure.
