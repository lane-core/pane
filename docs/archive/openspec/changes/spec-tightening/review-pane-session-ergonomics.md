# pane-session API Ergonomics Review

Assessed through the Be developer experience lens: "common things are easy to implement and the programming model is CLEAR" (Schillings, Newsletter #1-2).

---

## 1. Current API: The Ownership-Transfer Pattern

### What works

The `Chan<S, T>` typestate is the right primitive. It does for protocol correctness what BLooper did for threading -- makes the right thing structural rather than opt-in. The consuming-self pattern (`send` takes `self`, returns `Chan<S2, T>`) is the Rust-idiomatic way to express linear resource transfer. A developer who knows Rust will read this and understand it.

The crash safety property -- `Err(SessionError::Disconnected)` instead of panic -- is correct and load-bearing. This is the property that made us build BMessenger-based locks instead of raw pointer locks: identity-safe failure. The error type is clean, the `From` impls for io::Error and postcard::Error compose well, and the BrokenPipe/ConnectionReset/UnexpectedEof mapping to Disconnected is the right call.

### Where it gets painful

**Variable rebinding accumulates.** Look at the multi-step test:

```rust
let client = client.send("test-pane".to_string()).unwrap();
let (id, client) = client.recv().unwrap();
let client = client.send(vec![1u8, 2, 3]).unwrap();
let (ack, client) = client.recv().unwrap();
client.close();
```

Five steps, five `let client = ...` rebindings, five `.unwrap()` calls. For a test this is fine. For a real handshake protocol -- the one in the Maty integration plan has 4+ steps with branching -- this becomes a wall of rebinding that obscures the protocol's shape.

Compare to what a BLooper-based protocol looked like in BeOS:

```cpp
BMessage reply;
messenger.SendMessage(&hello, &reply);
// reply is populated, same variable, check reply.what
```

The BMessage pattern was mutable-in-place. You sent a message, got a reply in an output parameter, checked a status code. No ownership transfer, no rebinding. It was less safe -- nothing stopped you from using a stale reply -- but it was readable at a glance. A new developer could see the protocol shape without decoding the type gymnastics.

The question for pane: can we preserve the safety of ownership transfer while reducing the visual noise?

**Error handling doesn't compose over sequences.** Every step is `Result<Chan<S2, T>, SessionError>`, which means every step needs `?` or `.unwrap()`. For a 5-step handshake, that's 5 potential early returns, each identical in handling (all are Disconnected in practice). The `?` operator helps but still fragments the protocol into individual fallible steps rather than expressing it as a unit.

In BeOS, `SendMessage` returned a `status_t` and you checked it once per round-trip. The error model was coarser but easier to reason about.

**The recv tuple is awkward.** `let (value, chan) = chan.recv()?;` forces destructuring at every receive. The value comes first, the continuation comes second. This is the right type signature (it matches the linear logic -- the value is the witness, the continuation is the proof term) but it reads inside-out compared to the send direction where you pass the value and get back the continuation.

### Severity: moderate

The pattern is sound and idiomatic Rust. It will not surprise a Rust developer. But it fails the Schillings test for protocols longer than 3 steps -- you need to "know hundreds of details" (specifically, the rebinding chain) to trace what's happening. For kit-level API that non-expert developers use, this needs helpers.

---

## 2. Proposed Select/Branch

### What works

The Maty integration plan's `Select<L, R>` / `Branch<L, R>` with `select_left()` / `select_right()` / `offer()` returning `BranchResult<Chan<L, T>, Chan<R, T>>` is clean for the binary case. The wire encoding (1-byte tag) is minimal. The duality is correct: `Select` dualizes to `Branch`, which is the standard (+)/(&) correspondence.

The `offer()` method returning an enum that must be matched exhaustively is the key property. This is what gives you what BeOS's `MessageReceived` gave you with message codes: you must handle every case. But with session types, the compiler enforces it rather than relying on a default clause.

### Where it breaks: nested binary choice

The plan acknowledges this: "For protocols with more than two branches, nest: `Select<A, Select<B, Select<C, End>>>`." Let me show what happens at the handshake's accept/reject, which is only 2 branches:

```rust
// From the Maty plan -- the client handshake type
type PaneHandshake = Send<ClientHello,
    Recv<ServerHello,
        Send<Capabilities,
            Branch<
                Recv<Accepted, Send<Ready, End>>,
                Recv<Rejected, End>,
            >>>>;
```

This is readable. Two branches. Now imagine capability negotiation with 4 outcomes (accepted, accepted-with-fallback, version-mismatch, rejected):

```rust
type PaneHandshake = Send<ClientHello,
    Recv<ServerHello,
        Send<Capabilities,
            Branch<
                Recv<Accepted, Send<Ready, End>>,
                Branch<
                    Recv<AcceptedFallback, Send<Ready, End>>,
                    Branch<
                        Recv<VersionMismatch, End>,
                        Recv<Rejected, End>,
                    >>>>>>;
```

The nesting is right-associated. The type definition is a staircase. Worse, the `offer()` calls nest too:

```rust
match chan.offer()? {
    BranchResult::Left(chan) => { /* accepted */ }
    BranchResult::Right(chan) => {
        match chan.offer()? {
            BranchResult::Left(chan) => { /* fallback */ }
            BranchResult::Right(chan) => {
                match chan.offer()? {
                    BranchResult::Left(chan) => { /* version mismatch */ }
                    BranchResult::Right(chan) => { /* rejected */ }
                }
            }
        }
    }
}
```

This is the Peano arithmetic of choice: encoding N as S(S(S(Z))). It's theoretically sound and practically hostile. At N=4 the nesting is annoying. At N=6 it's unreadable. The plan says "A macro for flat N-ary select!/offer! can be added later if the nesting is ergonomically painful, but start without it."

I disagree with "start without it." Here's why: the handshake protocol is the first thing a pane client developer writes. If their first experience with the session type system is a nested staircase, they will conclude the system is academic and work around it. Be's first developer experience lesson was that the demo app must be trivially simple. The `offer!` macro is not a convenience -- it is the API.

### Severity: high for N >= 3

Binary Select/Branch is fine as the primitive. But the kit-level API must provide flat N-ary choice, or the programming model fails the clarity test at the point where developers first encounter it.

---

## 3. Three-Phase Transition

### The `close_and_take()` question

The Maty integration plan proposes:

```rust
impl<T: Transport> Chan<End, T> {
    pub fn close_and_take(self) -> T { ... }
}
```

This consumes the session-typed channel at `End`, extracts the transport, and hands it back for the active phase. The plan recommends `ManuallyDrop<T>` internally.

**Is this natural or a hack?** It's natural. The session type's job is done -- it has verified the handshake. The transport is a physical resource that outlives the session. Returning it is resource reclamation, not an escape hatch. The `End` state is the proof that the session completed correctly; consuming it and yielding the transport is the logical next step.

Compare to BeOS: when you called `BView::RemoveSelf()`, the view was detached from its window and returned to you for reuse or deletion. The window's ownership ended, the object's lifetime continued. Same pattern.

**But the name is wrong.** `close_and_take` describes the mechanism, not the intent. A developer reading protocol code should see:

```rust
let transport = handshake_chan.finish()?;
```

Not:

```rust
let transport = handshake_chan.close_and_take();
```

`finish` says "the protocol is complete, give me what I need for the next phase." `close_and_take` says "do two things to the implementation." The Be API naming convention was always intent over mechanism: `PostMessage`, not `WriteToPortAndEnqueue`; `Lock`, not `AcquireSemaphoreOrAtomicIncrement`.

### The lifecycle gap

The plan describes the three-phase flow as:

1. Create `Chan<PaneHandshake, UnixTransport>` on per-pane thread
2. Run handshake (send/recv/branch)
3. `close_and_take()` to get transport
4. `into_stream()` to get `UnixStream`
5. Create `SessionSource::new(stream)` for calloop
6. Register with calloop
7. Active phase runs in calloop handler

That's 7 steps with 3 type transitions (`Chan -> T -> UnixStream -> SessionSource`). A developer must know that `Chan` wraps a `Transport`, that `UnixTransport` wraps a `UnixStream`, that `SessionSource` wraps a `UnixStream` differently, and that the handoff goes through `finish -> into_stream -> SessionSource::new`.

In BeOS, the equivalent was: `BWindow` is created, its thread runs, it processes messages. One object, one lifetime. The client/server split was invisible -- the Interface Kit library handled it.

The pane equivalent should be a single function that encapsulates the handshake-to-active transition, not 7 steps that expose every layer of the transport stack.

### Severity: moderate

`close_and_take` is correct. The name should change. The lifecycle needs a helper that hides the transport unwrapping.

---

## 4. What's Missing

### Can a developer write a correct pane client from the kit API alone?

No. Not yet. Here's what they'd need to know that isn't in the API:

**a. How to define a protocol type.** The type alias pattern (`type ClientProtocol = Send<String, Recv<u64, End>>`) is shown in tests but isn't documented as a pattern. A developer seeing `Chan<S, T>` needs to know that `S` is built by composing `Send`, `Recv`, `Select`, `Branch`, and `End`. There's no guide, no macro, no builder.

In BeOS, you didn't define the protocol. You defined your message codes (`B_MY_MESSAGE = 'myms'`), and the messaging system handled the rest. The protocol was implicit in the handler structure. This is fundamentally different from session types where the protocol is explicit and must be defined up front. That's a feature -- explicit protocols catch bugs -- but it means the definition experience must be excellent.

**b. How to pair endpoints correctly.** `memory::pair()` does this automatically with `Dual<S>`. But for unix sockets, you manually construct `Chan::new(transport)` with an explicit type annotation. If you get the type wrong, you get a runtime codec error, not a compile error. The test `unix_transport.rs` line 26 shows this:

```rust
let server: Chan<Recv<String, Send<u64, End>>, _> = Chan::new(transport);
```

The developer must manually write out the dual of the client's protocol. If they get it wrong, the compiler won't catch it -- the session type is a phantom, and the transport sends raw bytes. The error surfaces at runtime as `SessionError::Codec`.

This is exactly the kind of bug that made us add BMessenger's identity checking: a mismatch that the type system should catch but doesn't because the two sides are constructed independently.

**c. How to handle errors in a multi-step protocol.** The `?` operator propagates `SessionError` but doesn't tell you which step failed. In a 5-step handshake, "peer disconnected" after step 3 means something different from "peer disconnected" after step 1, but the error is identical. BeOS's `status_t` had the same problem, and it was a real pain for debugging.

**d. How to go from handshake to active phase.** The three-phase transition described in the Maty plan doesn't exist in code yet. A developer reading the current API sees `Chan`, `Send`, `Recv`, `End`, `close()`, and `SessionSource`. There's nothing that connects them into a lifecycle.

### What helpers would make the common case trivial?

Schillings' standard: you don't need to know hundreds of details to get simple things working. The simple thing here is "connect to the compositor, do the handshake, start sending content." That needs to be ~10 lines of code.

---

## 5. Concrete Recommendations

### 5a. Chain combinator for multi-step sequences

The variable rebinding problem can be solved with a `then` method that chains send/recv without requiring intermediate bindings:

```rust
/// Chain a send followed by a receive, returning the received value
/// and the continuation channel.
///
/// This is the common request-response pattern: send a value, wait
/// for a response.
impl<A, B, S, T> Chan<Send<A, Recv<B, S>>, T>
where
    A: Serialize,
    B: DeserializeOwned,
    T: Transport,
{
    /// Send a value and immediately receive a response.
    /// Equivalent to `chan.send(val)?.recv()` but avoids intermediate binding.
    pub fn request(self, value: A) -> Result<(B, Chan<S, T>), SessionError> {
        let chan = self.send(value)?;
        chan.recv()
    }
}
```

Usage:

```rust
// Before: 4 lines, 2 rebindings
let client = client.send("test-pane".to_string())?;
let (id, client) = client.recv()?;

// After: 1 line, 1 destructuring
let (id, client) = client.request("test-pane".to_string())?;
```

This is the Be approach: recognize the dominant pattern (request-response was 80% of BMessage usage) and make it a single call. `BMessenger::SendMessage(&msg, &reply)` was exactly this -- send, block for response, done.

`request` is the right name because it describes the intent (make a request, get a response) rather than the mechanism (send then recv).

### 5b. Flat N-ary choice types via macro

```rust
/// Define a flat N-ary choice type.
///
/// `choice!(A, B, C)` expands to `Select<A, Select<B, C>>` (selector side)
/// with a corresponding `offer!` match macro.
macro_rules! choice {
    ($a:ty, $b:ty) => { Select<$a, $b> };
    ($a:ty, $($rest:ty),+) => { Select<$a, choice!($($rest),+)> };
}

/// Pattern-match on a flat N-ary offer.
///
/// Usage:
/// ```
/// offer!(chan, {
///     Accepted(chan) => { /* handle accepted */ },
///     Fallback(chan) => { /* handle fallback */ },
///     Rejected(chan) => { /* handle rejected */ },
/// })
/// ```
///
/// Expands to nested `offer()` calls with correct left/right dispatch.
/// The compiler verifies exhaustiveness through the macro's structure.
macro_rules! offer {
    // Base case: two arms
    ($chan:expr, { $label_a:ident($a:ident) => $body_a:expr, $label_b:ident($b:ident) => $body_b:expr $(,)? }) => {
        match $chan.offer()? {
            BranchResult::Left($a) => $body_a,
            BranchResult::Right($b) => $body_b,
        }
    };
    // Recursive case: first arm + rest
    ($chan:expr, { $label_a:ident($a:ident) => $body_a:expr, $($rest:tt)+ }) => {
        match $chan.offer()? {
            BranchResult::Left($a) => $body_a,
            BranchResult::Right(__rest_chan) => {
                offer!(__rest_chan, { $($rest)+ })
            },
        }
    };
}
```

Now a 4-way handshake result reads:

```rust
type HandshakeResult = choice!(
    Recv<Accepted, End>,
    Recv<AcceptedFallback, End>,
    Recv<VersionMismatch, End>,
    Recv<Rejected, End>,
);

// Offering side:
offer!(chan, {
    Accepted(chan) => {
        let (accepted, chan) = chan.recv()?;
        chan.close();
        Ok(Phase::Active(accepted))
    },
    Fallback(chan) => {
        let (fb, chan) = chan.recv()?;
        chan.close();
        Ok(Phase::Active(fb.into()))
    },
    VersionMismatch(chan) => {
        let (err, chan) = chan.recv()?;
        chan.close();
        Err(HandshakeError::Version(err))
    },
    Rejected(chan) => {
        let (reason, chan) = chan.recv()?;
        chan.close();
        Err(HandshakeError::Rejected(reason))
    },
})
```

Flat. Readable. Exhaustive. The labels (`Accepted`, `Fallback`, etc.) are documentation hints -- the macro doesn't use them for dispatch, but they make the match arms self-documenting. The selection side gets a corresponding `select!` that maps variant names to left/right chains, but that can come later since the selector is usually the compositor (one place) not the client (many places).

### 5c. Rename `close_and_take` to `finish`

```rust
impl<T: Transport> Chan<End, T> {
    /// Complete the session and reclaim the transport.
    ///
    /// Call this at the end of a session-typed phase (e.g., after handshake)
    /// to get the underlying transport for reuse in the next phase.
    /// The session type has reached End -- the protocol is complete.
    pub fn finish(self) -> T {
        self.transport
    }

    /// Close the session, dropping the transport.
    ///
    /// Use this when no further communication is needed on this transport.
    pub fn close(self) {
        drop(self);
    }
}
```

Two methods, two intents. `finish()` = "I'm done with session typing, give me the transport for what comes next." `close()` = "I'm done entirely." The dual paths are explicit. In BeOS terms: `finish` is `BView::RemoveSelf()` (detach for reuse); `close` is destructor (done forever).

### 5d. Phase transition helper

```rust
/// Complete a session-typed handshake and prepare for the active phase.
///
/// Takes a Chan<End, UnixTransport> (the completed handshake), extracts
/// the unix stream, and returns a SessionSource ready for calloop registration
/// plus a writer handle for sending responses.
///
/// This is the handshake-to-active boundary. The session type verified
/// the handshake; calloop handles the active phase.
pub fn into_active_phase(
    chan: Chan<End, UnixTransport>,
) -> io::Result<(SessionSource, UnixStream)> {
    let transport = chan.finish();
    let stream = transport.into_stream();
    let writer = stream.try_clone()?;
    let source = SessionSource::new(stream)?;
    Ok((source, writer))
}
```

One function. Takes the completed handshake, returns what you need for the active phase. A developer writing a pane client sees:

```rust
// 1. Connect and handshake (session-typed)
let chan = Chan::new(UnixTransport::from_stream(stream));
let chan = chan.send(ClientHello { ... })?;
let (server_hello, chan) = chan.recv()?;
let chan = chan.send(my_capabilities)?;

// 2. Transition to active phase (one call)
let (source, writer) = offer!(chan, {
    Accepted(chan) => {
        let (accepted, chan) = chan.recv()?;
        into_active_phase(chan)?
    },
    Rejected(chan) => {
        let (reason, chan) = chan.recv()?;
        chan.close();
        return Err(reason.into());
    },
})?;

// 3. Register with event loop
event_loop.handle().insert_source(source, |event, _, state| {
    // active phase handler
})?;
```

That's 15 lines from connect to active. Schillings-grade.

### 5e. Protocol definition with type aliases and documentation pattern

Provide a documented convention for protocol definitions that makes the type readable:

```rust
/// The pane client handshake protocol.
///
/// ```text
/// Client                          Compositor
///   |-- ClientHello ------------------>|
///   |<----------------- ServerHello ---|
///   |-- Capabilities ----------------->|
///   |                                  |
///   |     (compositor decides)         |
///   |                                  |
///   |<-- [Accept] ---- Accepted -------|  -> active phase
///   |<-- [Reject] ---- Rejected -------|  -> close
/// ```
///
/// After acceptance, the transport transitions to the active phase
/// with typed enum messages (ClientToComp / CompToClient).
pub type PaneHandshake = Send<ClientHello,
    Recv<ServerHello,
        Send<Capabilities,
            Branch<
                Recv<Accepted, End>,    // accepted: finish -> active phase
                Recv<Rejected, End>,    // rejected: close
            >>>>;
```

The ASCII protocol diagram is not decoration -- it's the only way a developer who hasn't internalized session type notation can read the type. BeOS's API documentation always led with "what this does in human terms" before "how the API works." The protocol diagram is the human-terms version.

Convention: every protocol type alias gets an ASCII diagram showing the message flow. This should be a stated standard in the crate's module docs.

### 5f. Typed dual construction for unix sockets

The current API lets you construct `Chan::new(transport)` with any session type, which means you can silently create a mismatched pair. Add a constructor that enforces duality:

```rust
/// Create a pair of session-typed channels over a unix socket pair.
///
/// Returns (client, server) with automatically-derived dual types.
/// This is the unix-socket equivalent of `memory::pair()`.
pub fn unix_pair<S: HasDual>() -> io::Result<(
    Chan<S, UnixTransport>,
    Chan<Dual<S>, UnixTransport>,
)> {
    let (s1, s2) = std::os::unix::net::UnixStream::pair()?;
    Ok((
        Chan::new(UnixTransport::from_stream(s1)),
        Chan::new(UnixTransport::from_stream(s2)),
    ))
}
```

For the server-accepts-client case (which is the production path), provide a typed accept:

```rust
/// Accept a connection and create a session-typed channel.
///
/// The session type S is the *server's* session type (i.e., the dual
/// of the client's protocol). The caller specifies what protocol they
/// expect to speak.
pub fn accept_session<S>(
    listener: &UnixListener,
) -> io::Result<Chan<S, UnixTransport>> {
    let (stream, _addr) = listener.accept()?;
    Ok(Chan::new(UnixTransport::from_stream(stream)))
}
```

This doesn't add compile-time safety against mismatches across the socket (you can't -- the two processes are separate compilation units). But it makes the intent clear and provides the entry point that a developer reaches for. They don't need to know about `UnixTransport::from_stream` and `Chan::new` separately.

### 5g. Error context for multi-step protocols

Add a step-tracking wrapper for debugging:

```rust
/// A session channel that tracks protocol step names for error reporting.
///
/// Wraps Chan and decorates SessionError with the step name on failure.
/// Use during development and testing; strip for production if the
/// overhead matters (it's one string clone per operation).
pub struct TrackedChan<S, T: Transport> {
    inner: Chan<S, T>,
    step: &'static str,
}

impl<S, T: Transport> TrackedChan<S, T> {
    pub fn step(self, name: &'static str) -> Self {
        TrackedChan { inner: self.inner, step: name }
    }
}
```

The implementation is straightforward -- wrap every `send`/`recv`/`offer` to catch errors and attach the step name. This is the BeOS `PRINT` macro approach: lightweight instrumentation that you can enable per-module.

Whether this becomes a newtype wrapper or a debug-only feature flag is a later decision. The key insight is that "peer disconnected" without context is insufficient for debugging real handshake failures, and the developer shouldn't have to add manual tracing to every protocol step.

---

## Summary: Priority Order

| # | Recommendation | Severity | Effort | Rationale |
|---|---------------|----------|--------|-----------|
| 1 | `offer!` / `choice!` macros | High | 1 day | First-contact API for branching; blocks readable handshake code |
| 2 | `request()` combinator | Moderate | 0.5 day | Eliminates most rebinding noise; covers 80% pattern |
| 3 | Rename `close_and_take` to `finish` | Moderate | 0.5 day | Names should express intent, not mechanism |
| 4 | `into_active_phase` helper | Moderate | 0.5 day | Collapses 4-step transport unwrapping into 1 call |
| 5 | `unix_pair` / `accept_session` constructors | Moderate | 0.5 day | Removes manual transport/chan assembly from client code |
| 6 | Protocol type documentation convention | Low | 0.5 day | ASCII diagrams for every protocol type alias |
| 7 | `TrackedChan` error context | Low | 1 day | Debug aid; defer until handshake debugging is painful |

Items 1-5 should ship with Phase 3 (Select/Branch + three-phase protocol). They are not polish; they are the difference between a session type library and a session type kit. BeOS did not ship BLooper and tell developers "compose your own message dispatch by calling `ReadMessageFromPort` and `DispatchMessage` manually." It shipped `Run()`, which did that for you. The helpers here are the `Run()` of the session type system.

Item 6 is a convention, not code. Adopt it now and enforce it in code review.

Item 7 is genuinely deferrable. Build the handshake first; when debugging it hurts, add the tracking.

---

## Appendix: The Be Standard Applied

Schillings said the BeOS was fun to program because "most of the operating system design and application framework was done by people with experience in writing real programs." The test for pane-session is: can you write a real pane client with it?

The current primitives (Chan, Send, Recv, End) are like having BMessage but not BLooper. You have the data container and the type-level protocol, but not the runtime scaffolding that makes common patterns trivial. The recommendations above are the BLooper layer: `request()` is `SendMessage(&msg, &reply)`, `offer!` is the structured `MessageReceived` dispatch, `finish()` is the lifecycle transition, `into_active_phase` is `BWindow::Run()`.

The session type theory is solid. The crash safety is correct. What's missing is the layer that makes a kit developer productive without reading the theory. That's the layer Be always built -- and the one that made developers love the platform.
