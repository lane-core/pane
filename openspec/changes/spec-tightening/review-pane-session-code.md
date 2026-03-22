# Code Review: pane-session

Reviewed 2026-03-22. Every source and test file read.

---

## 1. Correctness — Does the typestate actually work?

**The core guarantee holds.** You cannot call `recv()` on a `Chan<Send<A, S>, T>`. The impl blocks are correctly scoped: `send()` is only implemented for `Chan<Send<A, S>, T>`, `recv()` only for `Chan<Recv<A, S>, T>`, `close()` only for `Chan<End, T>`. The compiler rejects invalid sequences. This is right.

**Duality is correct.** `Dual<Send<A, S>>` = `Recv<A, Dual<S>>`, `Dual<Recv<A, S>>` = `Send<A, Dual<S>>`, `Dual<End>` = `End`. The involution property holds: `Dual<Dual<S>>` = `S` for all well-formed session types. Verified by reading `dual.rs`.

**`advance()` is sound.** It moves the transport out of the old `Chan` and into a new one with a different phantom type. The old `Chan` is consumed (moved), so the caller cannot keep using the old state. No `Clone` is derived or implemented. Good.

**No escape hatches.** `Chan::new()` is public, which means you can construct a `Chan` with any session type over any transport. This is fine — you need it for the server side, which doesn't go through `memory::pair()`. The spec's "caller is responsible for ensuring the session type matches what the peer expects" comment is correct and honest. The real enforcement is that both sides define their protocol types and the dual relationship between them. A mismatched type won't compile-time fail (it'll be a runtime deserialization error), but this is inherent to any system where two processes share a socket. The session type isn't enforcing across the network — it's enforcing within each side's code. That's the right level of guarantee.

**One subtlety worth noting:** `Chan::new()` being public means you can construct `Chan<End, T>` from a fresh transport and immediately `close()` it, or construct `Chan<Recv<A, S>, T>` on a transport that's going to send. These are programmer errors, not type system holes — the same way you can pass the wrong file descriptor to `read()`. The protection is within a given codebase's protocol definition, and that's where it matters.

**Verdict: sound.** The typestate pattern delivers what it promises.

---

## 2. Crash Safety

This is the make-or-break property. Let me trace every path where a peer death could be detected.

### Memory transport

- `send_raw()`: `mpsc::Sender::send()` returns `Err(SendError)` if the receiver is dropped. Mapped to `SessionError::Disconnected`. **Correct.**
- `recv_raw()`: `mpsc::Receiver::recv()` returns `Err(RecvError)` if the sender is dropped. Mapped to `SessionError::Disconnected`. **Correct.**

No panic paths. Clean.

### Unix transport

- `send_raw()`: `write_all()` to a closed socket returns `io::Error` with `BrokenPipe` or `ConnectionReset`. The `From<io::Error>` impl maps these to `SessionError::Disconnected`. **Correct.**
- `recv_raw()`: `read_exact()` to a closed socket returns `io::Error` with `UnexpectedEof`. Also mapped to `Disconnected`. **Correct.**

The error mapping in `error.rs` lines 48-56 covers the three relevant error kinds: `BrokenPipe`, `ConnectionReset`, `UnexpectedEof`. These are the right ones.

**Gap: `ConnectionAborted` is not mapped to `Disconnected`.** On some platforms, a peer close can produce `ConnectionAborted` rather than `ConnectionReset`. This is uncommon for Unix domain sockets (more of a TCP thing), but it's cheap to add and eliminates a category of surprise.

### Calloop source

- `process_events()` calls `read_length_prefixed()`, checks for `UnexpectedEof`, `ConnectionReset`, `BrokenPipe`. On match, fires `SessionEvent::Disconnected` callback and returns `PostAction::Remove`. **Correct.**
- The `let _ = callback(SessionEvent::Disconnected, &mut ())` silently ignores the callback's return value on disconnect. This is fine — the source is being removed regardless.

### The `#[must_use]` on `Chan`

Good. This catches the case where someone writes `chan.send(x);` without binding the result — the continuation channel would be dropped, leaving the peer waiting. The warning isn't foolproof (you can still `let _ = chan;`), but it catches the common mistake.

### What about `close()` on `Chan<End, T>`?

`close()` drops the transport. For Unix transport, this drops the `UnixStream`, closing the socket. The peer's next `recv()` will get EOF/`Disconnected`. No message is sent for `End`. This is correct — `End` is a type-level marker, not a wire protocol message. Both sides agree (via the session type) that the conversation is over.

**But consider this:** if one side reaches `End` and closes, but the other side still has one more `recv()` to execute (because it's behind in processing), the `recv()` will get `Disconnected` — which is semantically wrong. This can't happen in a correctly-typed system (if both sides follow dual protocols, they reach `End` simultaneously), but it's worth noting that the crash path and the normal-completion path look identical on the wire. There's no way to distinguish "peer finished normally" from "peer crashed." For the compositor, this is probably fine — a clean exit and a crash both result in cleanup. But if you ever need to distinguish them, you'll need an explicit `End` message.

### Verdict: crash-safe, with one minor gap.

Add `ConnectionAborted` to the error mapping. Consider whether you'll ever need to distinguish clean exit from crash (probably not now, but flag it for later).

---

## 3. The Calloop Integration

This is the most concerning part of the crate.

### The blocking mode switch

```rust
self.reader.set_nonblocking(false)?;
let result = read_length_prefixed(&mut self.reader);
self.reader.set_nonblocking(true)?;
```

**This is unsound as written.** Here's why:

`SessionSource::new()` calls `stream.try_clone()`. On Unix, `try_clone()` calls `dup()`, which creates a new file descriptor pointing to the **same file description**. The O_NONBLOCK flag is a property of the file description, not the file descriptor. So when you call `set_nonblocking(false)` on `self.reader`, you are **also** making the fd inside `Generic` blocking.

In practice this might not cause visible problems right now because:
1. The `Generic` fd is only used for poll/epoll registration, not for reading
2. The blocking mode switch happens inside `process_events`, which is already in the calloop dispatch — so nobody else is trying to read the Generic's fd concurrently

But it's still wrong in principle, and it becomes a real bug if:
- calloop's internals ever read from the fd during event processing (unlikely but not contractually excluded)
- You switch to `Edge` mode where the semantics of nonblocking matter for correctness
- Any future code tries to do non-calloop I/O on the Generic's fd

**The right fix: don't clone the stream.** Use `UnixStream::pair()` or pass two separate streams. Or better: don't switch blocking modes at all. Read in non-blocking mode and handle `WouldBlock` properly, accumulating partial reads in a buffer.

### The single-message-per-dispatch problem

The current code reads exactly one length-prefixed message per `process_events` call. With `Level` mode, calloop will fire again if there's more data in the buffer. But there's a subtlety: `read_length_prefixed` does two `read_exact` calls (4-byte length, then N-byte body). If the length prefix has arrived but the body hasn't (partial message in the kernel buffer), the blocking mode switch means `read_exact` will **block the entire compositor event loop** waiting for the body.

For messages from a healthy client, this might be microseconds. For messages from a client that's under load, stuck in a page fault, or being debugged — this blocks the compositor. This is the exact class of problem that BeOS's app_server avoided by having per-client threads. The calloop integration is supposed to be the compositor's main loop; it cannot block.

**The right approach:** non-blocking reads with a per-source read buffer. Read what's available, accumulate, deliver complete messages. This is more code but it's the correct architecture for an event loop.

### What if `set_nonblocking(true)` fails after a successful read?

The second `set_nonblocking` call can fail (returns `Result`). If it does, the error propagates up and the stream is left in blocking mode. Subsequent calloop dispatches will wake up due to level triggering, call `process_events`, and the `set_nonblocking(false)` call is now a no-op (already blocking). The stream stays blocking permanently. This is a leak of blocking state that could eventually block the main loop on a future read.

### The `write_message` free function

`write_message()` in `calloop.rs` takes `&UnixStream` and does blocking `write_all`. This is called from inside the calloop dispatch callback (as shown in `calloop_integration.rs` line 54). Writing to a unix socket from the compositor's main thread is fine for small messages (kernel buffer absorbs them). But if the client isn't reading and the socket buffer fills up, `write_all` blocks the compositor main loop.

This needs to either: (a) be documented as only safe for small messages, or (b) use non-blocking writes with a per-source write buffer.

### Session types are not used in the calloop path

The calloop integration completely bypasses the session type system. `SessionSource` works with raw bytes. The callback in the test manually calls `postcard::from_bytes`. There's no `Chan` involved. This means the compositor's message handling is **untyped** — you get bytes and you deserialize them yourself.

This is acknowledged in the design (the spec says calloop is for the compositor side, session types are for the client side), but it's worth being explicit: the compositor side does NOT get the typestate protection. The session type guarantees only exist on the client thread side.

For Phase 3's per-pane threading model (one `Chan` per pane, on a dedicated thread), the calloop source would be the **dispatcher** that accepts connections and spawns per-pane threads, each of which gets a proper `Chan`. That architecture would restore session type guarantees on the server side. But right now, the calloop integration is essentially a raw byte pipe with framing.

### Verdict: needs rework before it bears weight.

The blocking mode switch is the kind of thing that works in tests and fails under load. The single-message-per-dispatch model is fragile. The lack of buffering means partial messages can block the compositor. These aren't fatal — the architecture is right, the implementation needs to mature. But don't build Phase 3 on top of this exact code.

---

## 4. The Transport Abstraction

### Is `send_raw`/`recv_raw` the right interface?

Yes, for now. The transport handles framing and delivery; the session layer handles serialization and type tracking. This is a clean separation. The alternative — having the transport handle serde — would couple the transport to the serialization format.

The interface is synchronous and blocking (`recv_raw` blocks until data arrives). This is correct for the client side (one thread per Chan, blocking is fine). It's problematic for the compositor side, which is why the calloop integration doesn't use `Transport` at all.

**For Phase 3:** you'll likely want a `Transport` variant or a separate trait for non-blocking transports. Something like:

```rust
trait NonBlockingTransport: Transport {
    fn try_recv_raw(&mut self) -> Result<Option<Vec<u8>>, SessionError>;
    fn as_fd(&self) -> BorrowedFd<'_>;
}
```

But don't add it now. Wait until the per-pane threading model clarifies whether the compositor side actually needs non-blocking `Chan` operations, or whether dedicated pane threads (with blocking `Chan`) make the non-blocking path unnecessary.

### Length-prefixed framing in UnixTransport

The framing is correct: 4-byte little-endian length prefix, then body. Little-endian is the right choice (matches the dominant platform).

**Bug: no maximum message size check.** `recv_raw` reads `u32::from_le_bytes(len_buf) as usize` and allocates that many bytes. A malicious or buggy peer can send `0xFFFFFFFF` as the length and cause a 4GB allocation attempt. This will either OOM-kill the process or return an allocation error that propagates as a panic (Vec allocation failures panic in standard Rust).

Fix: add a `MAX_MESSAGE_SIZE` constant (something like 16MB or whatever the largest reasonable message is) and reject messages exceeding it with a new `SessionError` variant or by mapping to `Disconnected`.

**Performance note:** `send_raw` does two `write_all` calls — one for the 4-byte prefix, one for the body. This means two system calls. Not a problem for correctness, but for high-frequency small messages, it's worth coalescing into a single write (stack-allocate a small buffer, copy prefix + body if body is small, single `write_all`). This is a micro-optimization; don't bother now, but note it.

**The same framing is duplicated** in `calloop.rs` (`read_length_prefixed` and `write_message`). This should be factored into a shared module. Right now, if you change the framing format (add a version byte, change the length encoding), you'd need to change it in two places.

### The `Transport: Sized` bound

The `Sized` bound is necessary because `Chan` stores `T` by value. This prevents `dyn Transport`, which is fine — you don't want dynamic dispatch on every message send/recv. If you ever need transport-polymorphic code, use generics, not trait objects.

### Verdict: solid foundation, needs the max-size guard and deduplication.

---

## 5. What's Missing for Phase 3+

### Choose/Offer (branching)

The spec says: "Branching uses standard Rust enums. Enum variants contain session continuations. Pattern matching is exhaustive."

The current crate has no `Choose` or `Offer` types. Here's what it needs:

```rust
/// Choose<L, R>: the sender picks left or right branch.
/// On the wire: sends a 0u8 (left) or 1u8 (right), then continues as L or R.
pub struct Choose<L, R>(PhantomData<(L, R)>);

/// Offer<L, R>: the receiver learns which branch the sender picked.
/// Returns an enum so the receiver can match on it.
pub struct Offer<L, R>(PhantomData<(L, R)>);
```

The `Chan<Choose<L, R>, T>` impl offers `left()` -> `Chan<L, T>` and `right()` -> `Chan<R, T>`. The `Chan<Offer<L, R>, T>` impl offers `offer()` -> `Result<Branch<Chan<L, T>, Chan<R, T>>, SessionError>`.

Duality: `Dual<Choose<L, R>>` = `Offer<Dual<L>, Dual<R>>`.

For more than two branches, either nest (`Choose<A, Choose<B, C>>`) or use an enum-based macro that generates N-ary branching. The nested approach is simpler; the macro is more ergonomic. Start with nesting, add the macro when you have real protocol definitions that need it.

**Impact on existing code:** additive. Nothing changes about `Send`/`Recv`/`End`. New types, new impls, new dual impls.

### Recursive protocols

This is harder. A heartbeat protocol looks like:

```
type Heartbeat = Recv<Ping, Send<Pong, Heartbeat>>;
```

This is an infinite type, which Rust doesn't support. The standard approach is:

```rust
pub struct Rec<S>(PhantomData<S>);   // enter a recursive scope
pub struct Var;                        // jump back to the enclosing Rec
```

With `Rec` and `Var`, the heartbeat becomes:

```
type Heartbeat = Rec<Recv<Ping, Send<Pong, Var>>>;
```

Implementation: `Chan<Rec<S>, T>` offers `enter()` -> `Chan<S, T>` (entering the loop body). `Chan<Var, T>` offers `recurse()` -> `Chan<S, T>` (jumping back). The recursion is achieved by having `Var` resolve to the nearest enclosing `Rec`'s type parameter. This requires some type-level machinery (a stack of recursive bindings) and is the trickiest part to get right.

**Alternatively:** don't use `Rec`/`Var`. Instead, have the loop be in application code:

```rust
let mut chan: Chan<Recv<Ping, Send<Pong, End>>, _> = /*...*/;
loop {
    let (ping, c) = chan.recv()?;
    chan = c.send(Pong)?;
    // chan is now Chan<End, _> — but we need to loop...
}
```

This doesn't work because the types don't line up. The workaround is to not use `End` in the loop body — use a separate "restart" mechanism. This is where the design gets subtle.

**My recommendation:** implement `Choose`/`Offer` first. Punt `Rec`/`Var` until you have a concrete protocol (the heartbeat protocol from the spec, or the pane lifecycle) that actually needs it. You might find that the real protocols are better expressed as a sequence of request-response pairs (each a separate session) rather than a single recursive session.

### Per-pane threading model

The spec says: "Dispatcher (1 per connection): Dedicated thread — demuxes socket I/O to per-pane threads."

What this needs from pane-session:

1. **Sub-session multiplexing.** One unix socket carries conversations for multiple panes. Each message needs a pane ID prefix (before the session-type payload). The dispatcher reads the pane ID, routes to the right per-pane thread.

2. **Chan per pane thread.** Each pane thread gets a `Chan` with the pane lifecycle session type. The transport for this `Chan` is NOT a raw unix socket — it's one end of a per-pane in-process channel (like `MemoryTransport` but from the dispatcher).

3. **The dispatcher itself** is the thing that interfaces with the socket. It reads frames, demultiplexes by pane ID, and forwards to per-pane `MemoryTransport` instances.

This means the architecture is:

```
Socket <-> Dispatcher thread <-> N x (MemoryTransport <-> Pane thread with Chan)
```

The dispatcher does the socket I/O and framing. The pane threads do the session-typed protocol. The `UnixTransport` is used by the client side (one socket per connection, one-to-one). The server side uses `MemoryTransport` between dispatcher and pane threads.

**Impact on existing code:** `MemoryTransport` is already there. The dispatcher is new code but doesn't change the session type primitives. You'll need a message framing layer that adds pane IDs — this sits between the socket and the `Transport`. The session type layer is unchanged.

---

## 6. API Ergonomics

The usage pattern from the tests reads well:

```rust
let client = client.send("hello".to_string()).unwrap();
let (response, client) = client.recv().unwrap();
client.close();
```

This is idiomatic Rust. The variable shadowing (`let client = client.send(...)`) is natural for state machine transitions. The tuple return from `recv()` is a standard pattern.

**Friction points:**

1. **The `memory::pair()` return type** requires annotation: `let (client, server): (Chan<ClientProtocol, _>, _) = memory::pair();`. The `_` for the server side is good (you get the dual automatically), but the full annotation on the client is verbose. Consider a type alias: `type Session<S> = Chan<S, MemoryTransport>`. Users can then write `let (client, server): (Session<ClientProtocol>, _) = memory::pair();`. Minor.

2. **The unix side requires manual type annotation.** In `unix_transport.rs` line 26: `let server: Chan<Recv<String, Send<u64, End>>, _> = Chan::new(transport);`. The server has to manually write out the dual of the client's protocol. This is where a `Dual` type alias helps: `let server: Chan<Dual<ClientProtocol>, _> = Chan::new(transport);`. But the user needs to import `Dual` and the client protocol type. This is inherent to the cross-process nature of the thing — you can't have type inference across a socket.

3. **`close()` is redundant with `drop()`.** `Chan<End, T>::close(self)` just calls `drop(self)`. The explicit `close()` is good documentation ("I intentionally ended this session"), but a `Drop` impl that's a no-op would also work. The `#[must_use]` catches the case where you forget. This is fine as-is.

4. **Error handling could use `?` more naturally** if `SessionError` implemented `Into<Box<dyn Error>>`. It already implements `std::error::Error`, so this works. But the tests all use `.unwrap()`, which is fine for tests but means we haven't seen what the error ergonomics look like in real code. Worth a test that uses `?` with a custom error type.

**Overall: clean, natural API.** The patterns are exactly what I'd expect from a Rust state machine library. No surprises, no ceremony.

---

## 7. Test Coverage

### What's covered

- Simple request-response (memory): yes
- Multi-step protocol (memory): yes
- Crash at start (memory): yes (drop server before any exchange)
- Crash mid-conversation (memory): yes
- Simple request-response (unix): yes
- Multi-step protocol (unix): yes
- Crash mid-conversation (unix): yes
- Server panic recovery (unix): yes
- Calloop message receipt: yes
- Calloop crash detection: yes

### What's NOT covered

1. **Large messages.** No test sends anything near the u32 max. A test that sends, say, 1MB would exercise the framing under realistic conditions. A test that sends >4GB (to exercise the `as u32` truncation) would catch the overflow bug in `send_raw`.

2. **Empty messages.** What happens if you send a zero-length postcard payload? The length prefix would be 0, `recv_raw` would allocate a zero-length Vec and return it, postcard would try to deserialize zero bytes. This might produce a `Codec` error, which is fine, but it's worth a test to verify.

3. **Rapid message sequence.** Send 1000 messages in sequence. This exercises whether the framing stays aligned — one byte off and everything after is corrupt.

4. **Concurrent sends/recvs on different panes** (when multiplexing exists). Not applicable yet.

5. **Calloop with multiple messages before dispatch.** The current crash test relies on two dispatches. A test that sends 5 messages before the first dispatch would verify that level triggering correctly re-fires.

6. **Transport error paths.** No test manufactures an `Io` error (as opposed to `Disconnected`). What happens if the underlying socket returns `ENOMEM` or `EINTR`? `EINTR` in particular is interesting — `read_exact` in std handles it (retries), but it's worth verifying.

7. **The `Codec` error path.** Send valid-length garbage bytes and verify you get `SessionError::Codec`, not a panic.

8. **proptest is in dev-dependencies but unused.** This is exactly the right tool for testing framing correctness (generate random byte sequences, verify that send-then-recv roundtrips are identity). Write that test.

### Verdict: good coverage for a Phase 2 deliverable. Notable gaps for hardening.

---

## 8. Performance

### Blocking mode switch per message (calloop)

This is two `fcntl` system calls per message (set blocking, set nonblocking). On Linux, `fcntl` is cheap (~100ns), so for modest message rates (100/s) this is irrelevant. At 10,000 messages/s it's 2ms/s of overhead. Not a bottleneck, but unnecessary — the right architecture (non-blocking reads with buffering) eliminates it entirely.

### Two writes per send (unix transport)

`send_raw` does `write_all(&len)` then `write_all(data)`. Each `write_all` is at least one system call. For a 100-byte message, you're doing 2 syscalls instead of 1. At high message rates, this matters.

Fix: `writev` (scatter-gather I/O) sends both in one syscall. Or just concatenate into a single buffer:

```rust
let mut frame = Vec::with_capacity(4 + data.len());
frame.extend_from_slice(&(data.len() as u32).to_le_bytes());
frame.extend_from_slice(data);
self.stream.write_all(&frame)?;
```

The allocation is unfortunate but cheaper than a second syscall. Or use a small stack buffer with `iovec`.

### Vec allocation per recv

Every `recv_raw` allocates a new `Vec<u8>`. For small frequent messages, this puts pressure on the allocator. A reusable buffer (stored in the transport) would eliminate this. But don't optimize this until you have profiling data — the allocator is fast for small allocations.

### postcard serialization

postcard is designed for embedded; it's very fast and produces compact output (varint encoding). Good choice. No concerns here.

### Verdict: no showstoppers. The two-write issue is the most impactful and easiest to fix.

---

## Summary — severity-ranked findings

### Critical (fix before building on this)

1. **No maximum message size in `recv_raw`.** A malformed or malicious length prefix causes unbounded allocation. Add a `MAX_MESSAGE_SIZE` constant and reject oversized messages. (`unix.rs:37`, `calloop.rs:109`)

2. **Calloop blocking mode switch affects the Generic's fd.** `try_clone()` + `set_nonblocking()` operates on the shared file description. Replace with non-blocking reads and a per-source accumulation buffer. This also fixes the partial-message-blocks-compositor problem. (`calloop.rs:76-78`)

### Moderate (fix before Phase 3)

3. **Calloop integration bypasses session types entirely.** The compositor side has no typestate protection. This is architecturally acknowledged (per-pane threads will get `Chan`), but the current calloop path should at minimum be documented as "raw dispatch layer, not session-typed" — someone will mistake it for the session layer.

4. **Framing logic duplicated** between `unix.rs` and `calloop.rs`. Factor into a shared `framing` module. One definition of the wire format.

5. **`ConnectionAborted` not mapped to `Disconnected`** in `error.rs`. Add it to the match arm.

6. **`send_raw` truncates silently on messages > 4GB.** `data.len() as u32` wraps on overflow. In practice, postcard won't produce 4GB output, but the cast should either be checked (`u32::try_from(data.len()).map_err(...)`) or documented as assuming sub-4GB messages.

### Minor (improve when convenient)

7. **Two `write_all` calls per send.** Coalesce length prefix and body into a single write.

8. **proptest dependency is unused.** Write property-based roundtrip tests for the framing layer.

9. **No test for the `Codec` error path** (deserializing garbage).

10. **`write_message` in calloop does blocking writes from the main loop.** Document the assumption (small messages only) or add buffered non-blocking writes.

---

## Be's perspective

At Be, the app_server had per-client threads. Each `ServerApp` ran its own `BLooper` on its own thread, reading from a `BPort` with its own message queue. The compositor main loop (the drawing/compositing thread) never blocked on client I/O — that was the `ServerApp`'s job. The `ServerApp` translated client requests into drawing commands that were queued to the compositor.

The pane architecture gets this right in the spec: per-pane threads with `Chan` for the protocol, calloop for the compositor's drawing/Wayland loop. The current `SessionSource` is trying to do client I/O from the compositor main loop, which is the wrong threading level for production. It's fine as a Phase 2 proof of concept (it proves calloop can see session messages), but the production path is: calloop accepts connections, spawns dispatcher threads, dispatcher spawns pane threads, pane threads use `Chan`. The calloop path touches protocol bytes only at the connection-accept level.

The bones here are right. The session type primitives are correct. The crash safety guarantee holds. The unix transport works. What needs work is the compositor-side integration, and that work is already planned (the per-pane threading model). Don't let the calloop issues distract from the fact that the core `Chan<S, T>` abstraction is solid.

Ship the primitives. Fix the max-message-size guard. Plan the calloop rework as part of the per-pane threading implementation in Phase 3.
