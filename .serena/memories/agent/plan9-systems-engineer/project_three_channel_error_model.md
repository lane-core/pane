---
name: Three-channel error model decision
description: Handler returns bare Flow (not Result<Flow>); errors separated into protocol (ReplyPort), control (Flow), and crash (panic/catch_unwind) channels
type: project
---

Resolved the Result<Flow> vs bare Flow debate (2026-04-03). Plan 9 engineer withdrew dissent after three-channel decomposition.

## The three channels

1. **Protocol channel** — ReplyPort::reply(error) or drop(ReplyPort) → ReplyFailed. This IS 9P Rerror: error bound to a specific request, handler continues. ServiceLost for service disconnects.
2. **Control channel** — Flow::Continue / Flow::Stop. Handler tells looper about lifecycle only. No error information.
3. **Crash channel** — panic → catch_unwind at looper boundary. Handler cannot continue. State potentially corrupt.

## Key decision: Handler methods return `Flow`, not `Result<Flow>`

- Handler owns its error domain internally (Result in process_key(), etc.)
- Per-event errors caught and handled by handler, not propagated to looper
- Protocol errors go through ReplyPort (channel 1), not return type
- Looper has no business handling handler-domain errors

## ExitReason semantic tightening

With bare Flow, ExitReason::Failed means exclusively "caught panic" — handler state is potentially corrupt. Previously it also covered handler Err returns (which could be benign config errors). The new semantic is more precise:
- Graceful = Flow::Stop (handler chose to exit, may have encountered errors it handled)
- Disconnected = connection loss
- Failed = caught panic (handler is broken)
- InfraError = transport/framing failure

## Implementation requirement

`panic = unwind` must be set for the looper crate (at minimum). `panic = abort` would eliminate channel 3 entirely — panics become process death, not caught failures. This is load-bearing.

**Why:** Lane presented three-channel decomposition to resolve tension between Plan 9 (Result<Flow>), Be (panic), and session-type (typed error) positions. All three concerns are addressed by orthogonal channels.

**How to apply:** Update Handler trait signatures from `-> anyhow::Result<Flow>` to `-> Flow`. Update ExitReason docs to clarify Failed = caught panic. Ensure panic = unwind in Cargo.toml.
