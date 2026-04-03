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
- Three-phase protocol: session-typed handshake → per-service typed messages → session-typed teardown
- Message is base-protocol-only (lifecycle + display). Clone-safe. Service events dispatch through Handles<P>.
- Messenger wraps scoped Handle + ServiceRouter. Cloneable Send handle.
- Handler (lifecycle) + DisplayHandler (display) + Handles<P> (services)
- Protocol trait links ServiceId + Message type. Protocol::Message requires Serialize + DeserializeOwned.
- FilterChain applies in registration order, any filter can consume or transform
- ExitReason distinguishes HandlerExit / CompositorClose / Disconnected
- Result<Flow> return convention. Flow::Continue / Flow::Stop. Err = actor failure.

## Testing
- proptest for roundtrip serialization tests
- Tests run on macOS against in-memory channels
