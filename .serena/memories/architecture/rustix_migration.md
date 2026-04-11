---
type: architecture
status: current
supersedes: [pane/rustix_migration]
created: 2026-04-07
last_updated: 2026-04-10
importance: normal
keywords: [rustix, peer_cred, FFI, unsafe, OwnedFd, BorrowedFd, pane-session, SO_PEERCRED, getpeereid]
related: [architecture/looper]
agents: [pane-architect]
---

# rustix migration for pane-session

## Summary

Replace hand-rolled FFI in pane-session with rustix (crates.io,
v1.1+). The peer_cred.rs module has `extern "C"` declarations for
`getsockopt`, `getpeereid`, `getuid` with raw pointer casts and
unsafe blocks. rustix provides safe typed wrappers for all of
these.

## Why

- Eliminates unsafe blocks in peer_cred.rs (SO_PEERCRED,
  getpeereid, LOCAL_PEERPID)
- Handles Linux / macOS platform branching internally
- Provides OwnedFd / BorrowedFd for type-safe fd management across
  pane-session
- Shared dependency with psh (the pane system shell uses rustix
  for fork/pipe/dup2), so the dep is already in Lane's mental
  budget for the broader workspace

## Scope

| File | Change |
|---|---|
| `pane-session/src/peer_cred.rs` | Replace `extern C` FFI with `rustix::net::sockopt` and `rustix::process` |
| `pane-session/src/server.rs` (when built) | socketpair, listen, accept via rustix |
| `pane-session/src/transport.rs` | Potential OwnedFd interop for UnixStream |

Does NOT affect pane-proto or pane-app (no syscalls in those crates).

## How to apply

1. Add `rustix = { version = "1", features = ["net", "process"] }`
   to `pane-session/Cargo.toml`
2. Rewrite `peer_cred.rs` to use `rustix::net::sockopt::get_socket_peercred()`
   (Linux) and `rustix::process::getuid()`
3. Remove the `extern C` blocks and manual `#[cfg]` platform
   branching

## Status

Not yet implemented. Tracked in `PLAN.md` under pane-session.
