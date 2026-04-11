# Vertical Slice: First Running Pane (decided 2026-04-05)

Lane chose Path B over protocol-bottom-up. Goal: a real hello-world pane that connects to a stub server over a unix socket, completes the handshake, receives lifecycle messages, and exits.

## Why
- Ergonomics review said "no entry point, no apps" is the critical gap
- Flushes out integration issues between types designed in isolation
- Forces Framework protocols, Handshake types, ConnectionSource, and Looper to be built because they're needed, not because they're next on a list

## What "done" looks like
A binary (pane-hello or a test) that:
1. Starts a stub server listening on a unix socket
2. Connects via `pane::connect("com.pane.hello")`
3. Server derives PeerAuth from SO_PEERCRED
4. Client sends Hello, server sends Welcome (via FrameCodec)
5. Client enters active phase, handler receives Ready
6. Handler calls messenger.set_content(b"Hello, world")
7. Handler returns Flow::Stop on CloseRequested
8. Destruction sequence runs, connection closes cleanly

## What must be built (rough dependency order)
1. Framework protocols (Display as Protocol, ControlMessage) — pane-proto
2. Handshake types (Hello, Welcome, ServiceInterest, ServiceBinding) — pane-proto or pane-session
3. ConnectionSource — calloop EventSource wrapping a unix socket fd
4. Looper — calloop-backed, replacing LooperCore's manual pump
5. pane::connect() — entry point that does transport + handshake
6. Stub pane-server — minimal unix socket listener, handshake, lifecycle dispatch
7. pane-hello — the binary

## Design principle
Build the minimum to make the slice work. Don't gold-plate the server — it's a stub that will be replaced. Do get the client-side types right because they're the developer API.
