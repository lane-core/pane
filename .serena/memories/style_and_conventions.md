# Code Style & Conventions

## Rust
- Standard `cargo fmt` formatting
- Clippy with `-D warnings`
- Derive order: `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize` (as applicable)
- `pub(crate)` for internal APIs, `#[doc(hidden)] pub` for test-support types
- Comments explain *why*, not *what*
- No unnecessary abstractions — three similar lines better than premature abstraction
- Error handling from the start (Result types, not panics)
- Session types use crash-safe `SessionError::Disconnected`, never panic on drop

## Architecture Patterns
- Per-pane threading (BeOS BLooper model)
- Three-phase protocol: session-typed handshake → typed enum active phase → session-typed teardown
- Message is flattened from CompToClient (PaneId stripped, nesting eliminated)
- Messenger (BMessenger equivalent) is cloneable Send handle
- FilterChain applies in registration order, any filter can consume
- ExitReason distinguishes HandlerExit / CompositorClose / Disconnected

## Testing
- proptest for roundtrip serialization tests
- MockCompositor for integration tests (no real compositor needed)
- Tests run on macOS against in-memory channels
