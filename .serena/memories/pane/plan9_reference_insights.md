# Plan 9 Reference Material: Insights for Pane Implementation

Extracted from vendored Plan 9 Programmer's Manual and system papers in `reference/plan9/`.
Each section maps specific primary-source findings to pane's unimplemented subsystems.

---

## 1. pane-fs (Synthetic Filesystem)

### Namespace construction (namespace(6), bind(1))

The namespace file format (`reference/plan9/man/6/namespace`) defines six operations:
- `mount [-abcC] servename old [spec]` — attach a file server to a directory
- `bind [-abcC] new old` — alias a file/directory to another location
- `import [-abc] host [remotepath] mountpoint` — import remote namespace
- `cd dir`, `unmount [new] old`, `clear` (rfork RFCNAMEG), `. path` (include)

**Bind/mount flags** (from `reference/plan9/man/1/bind`):
- No flag: REPLACE — old is entirely replaced by new
- `-b`: MBEFORE — new directory prepended to union (searched first)
- `-a`: MAFTER — new directory appended to union (searched last)
- `-c`: MCREATE — permit file creation in this union element (creation goes to first element with -c flag)
- `-C`: cache — kernel may cache read data; currency verified at each open

**Key insight for pane-fs:** Plan 9's union directories have MBEFORE/MAFTER ambiguity — search order matters, and the order can't be discovered by inspection. pane-fs avoids this entirely by using computed projections over an indexed store (BFS query model). This is documented in the divergences tracker but the primary source confirms why: `namespace(4)` shows the conventional namespace has `/bin` as a union of `/$objtype/bin`, `/rc/bin`, `$home/$objtype/bin` etc. — and the order matters for which binary you get. pane-fs should never have this problem because its directories are filter results, not overlays.

**Dynamic content in synthetic filesystems:** The names paper (`reference/plan9/papers/names.ms`) is explicit: "Their contents are synthesized on demand when read; when written, they cause modifications to kernel data structures." `/dev/time`, `/dev/pid`, `/dev/user` etc. are all computed on read. Text format throughout — "All these files contain text, not binary numbers, so their use is free of byte-order problems."

**pane-fs implication:** pane-fs files like `/pane/<id>/attrs/title` should return text, computed on read from pane-store's index. Write to `/pane/<id>/ctl` should route commands to the owning pane via the protocol. This is exactly the proc(3) ctl pattern.

### The proc(3) ctl pattern

From `reference/plan9/man/3/proc`: The `ctl` file accepts textual commands: `stop`, `start`, `kill`, `hang`, `private`, `close N`, `pri N`, `wired N`, etc. Multiple commands on multiple writes. Error returned if command inappropriate for current state.

**Key detail:** `waitstop` blocks the *writer* until the target process enters Stopped state. This is a synchronous completion mechanism via the filesystem — write blocks until the effect is confirmed.

**pane-fs implication:** `/pane/<id>/ctl` should accept commands like `focus`, `hide`, `unhide`, `close`, `resize minx miny maxx maxy`. Consider whether some commands should block until acknowledged (like `waitstop`). For scripts, blocking confirmation is natural; for interactive use, fire-and-forget is better. Offer both: `ctl` for fire-and-forget, potentially a `wait` file or a convention where certain commands block.

### The rio(4) wctl file

From `reference/plan9/man/4/rio`: `wctl` is both readable and writable. Reading returns window geometry + state ("hidden"/"visible", "current"/"notcurrent"). A subsequent read *blocks until window changes size, location, or state*. Writing accepts commands: `resize`, `move`, `scroll`, `noscroll`, `top`, `bottom`, `hide`, `unhide`, `current`, `delete`, `new`.

**This is the Plan 9 observer pattern.** There is no subscription mechanism — you block on read and get woken when state changes. Simple, correct for scripting, but doesn't scale to multiple observers (each needs its own fd, each blocks a thread/goroutine).

**pane-fs implication:** pane-fs should expose a blocking-read `wctl`-equivalent file per pane. This is the scripting-tier observer — external tools `cat /pane/<id>/event` and block. For the protocol tier, use the push-based observer pattern already planned (Messenger::start_watching). Both coexist.

### The namespace(4) conventional structure

From `reference/plan9/man/4/namespace`: After bootstrap, the conventional namespace has:
- `/srv` — service registry (srv(3))
- `/mnt/factotum` — authentication agent
- `/mnt/wsys` — window system mount point
- `/mnt/term` — terminal namespace as seen by CPU server
- `/n/kremvax` — mount point for remote machine

**pane-fs implication:** Following this convention, pane should establish:
- `/pane/` — pane hierarchy (computed)
- `/srv/pane/` — service registry for pane services
- `/mnt/pane/` — mount point for pane-fs (if not at `/pane/` directly)

---

## 2. pane-roster (Service Discovery)

### srv(3) kernel device

From `reference/plan9/man/3/srv`: srv is "a bulletin board on which processes may post open file descriptors to make them available to other processes." Create a file in `/srv`, write a file descriptor number to it as text. Anyone can open the file to get a reference to that fd. The file holds a reference even if no process has it open. Removing the file releases the reference.

**Critical limitation:** No lifecycle management. If a server crashes, the `/srv` entry persists as a stale reference. There is no garbage collection, no health checking, no "is this service still alive" query. The entry just sits there.

**Plan 9 workaround:** Users manually clean up `/srv` entries. On reboot, `/srv` is empty (it's an in-memory device). For long-running systems, stale entries accumulate.

**pane-roster implication:** This is exactly why pane-roster must actively monitor service health via init system integration, not passively hold fd references. The divergence tracker already notes this. The primary source confirms: srv(3) is a passive bulletin board. pane-roster needs `query_liveness()` against the init system (s6/launchd/systemd) to determine if a posted service is still running. Init system integration is not optional — it's the fix for srv(3)'s known deficiency.

### srv(4) — the user-level srv command

From `reference/plan9/man/4/srv`: `srv` dials a remote machine, establishes a 9P connection, and posts it in `/srv`. `9fs` is a convenience wrapper. This is the mechanism by which remote file servers become locally available.

**pane-roster implication for federation:** When a remote pane instance is discovered, pane-roster should make it available analogously — not by posting an fd, but by registering it in the roster's index so that pane-fs can serve queries that include remote panes. The federation protocol should handle the equivalent of `srv`'s dial + post + mount in a single operation.

---

## 3. Observer Pattern (Property Watching)

### Plan 9 had NO subscription/notification protocol

9P has no built-in notification mechanism. The protocol is entirely request-response. Plan 9's approach was **blocking reads**: you read a file, and if nothing has changed, the read blocks until something does.

**Examples from primary sources:**
- `rio(4)` wctl: "A subsequent read will block until the window changes size, location, or state."
- `proc(3)` wait: "If the process has no extant children, living or exited, a read of wait will block."
- `mouse(3)` via rio(4): "Reading the mouse file blocks until the mouse moves or a button changes."

**The pattern:** State-change notification is a blocking read on a per-resource file. Each file represents one observable. The read returns when the state changes. No subscription setup, no teardown, no multiplexing within the protocol — multiplexing is done by the client (using threads, or libthread's `alt`).

**Limitation:** This requires one thread per observable per observer. For a small number of observers watching a small number of properties, it works. For a reactive UI framework where dozens of properties need to be watched simultaneously, it doesn't scale well.

**pane observer implication:** The dual-path model is correct:
1. **Filesystem tier:** Blocking-read files like rio's wctl for external tools. Scripts that `cat /pane/<id>/event` get woken on change.
2. **Protocol tier:** Push-based `start_watching(property, watcher)` for kit-level reactive UI. This is what Plan 9 never had.

### The plumber as proto-observer

From `reference/plan9/man/4/plumber` and `reference/plan9/papers/plumb.ms`: The plumber provides *implicit* data routing, not property-change notification. Messages go to the `send` file and are dispatched to ports based on pattern-matching rules. Applications read from their port and block until a message arrives.

**Key detail from plumber(4):** "A copy of each message is sent to each client that has the corresponding port open." — This is multicast! Multiple readers on the same port each get a copy. And: "If none has it open, and the rule has a `plumb client` or `plumb start` rule, that rule is applied" — lazy application startup.

**Key detail from plumb(6):** The `click` attribute is crucial — it carries cursor context, allowing the plumber to narrow a selection using regex rules + click position. "The matches verb has special properties that enable the rules to select which portion of the data is to be sent to the destination."

**pane routing implication:** pane's kit-level routing should support:
1. Content-based dispatch (like plumber rules)
2. Click/cursor context refinement (the `click` attribute pattern)
3. Multicast to all interested receivers when a port has multiple listeners
4. Lazy app start when no listener exists for a destination

**Plumber BUGS section:** "Plumber's file name space is fixed, so it is difficult to plumb messages that involve files in newly mounted services." — This is because the plumber evaluated rules in its own (fixed) namespace. pane avoids this by evaluating routing rules in the application's own namespace (kit-level, not central server).

---

## 4. pane-shell (Terminal Emulator)

### 8½/rio architecture (8½ paper + rio(4))

From the 8½ paper (`reference/plan9/papers/8½/8½.ms`):

**Core architecture:** 8½ is a file server that multiplexes `/dev/cons`, `/dev/mouse`, `/dev/bitblt` etc. Each window gets its own instance of these files via per-process namespaces. "The environment 8½ provides its clients is exactly the environment under which it is implemented." — This recursive symmetry is what allowed 8½ to run inside itself.

**Window creation mechanism:** When 8½ creates a new window:
1. Fork child process
2. Child duplicates its namespace (rfork RFNAMEG)
3. Child mounts 8½'s pipe onto `/dev` with MBEFORE flag — this shadows existing `/dev/cons` etc. with per-window versions
4. Child opens `/dev/cons` three times for stdin/stdout/stderr
5. Child execs the shell

"This entire sequence, complete with error handling, is 33 lines of C."

**External window creation:** 8½ posts its service pipe to `/srv`. External processes mount it with a spec string containing window dimensions. The mount itself creates the window.

**pane-shell implication:** pane-shell doesn't need to replicate the namespace trick (we're on Linux, not Plan 9). But the design principle applies: the terminal should be indistinguishable from any other pane at the protocol level. It communicates with the compositor via the same session-typed channels. The VT parser and PTY bridge are internal to pane-shell; the compositor sees standard pane protocol messages.

### rio(4) file hierarchy per window

From `reference/plan9/man/4/rio`:
- `cons` — virtual terminal (read for keyboard input, write for output)
- `consctl` — mode control (rawon/rawoff/holdon/holdoff). "Closing the file makes the window revert to default state."
- `label` — window label (read/write), used as tag when hidden
- `mouse` — virtual mouse, opening turns off scrolling/editing/menus
- `snarf` — clipboard content (read returns contents, write sets them)
- `text` — full window contents (read-only)
- `wctl` — geometry + state (readable, blocks on change; writable for commands)
- `wdir` — working directory (read/write, used for plumb messages)
- `winid` — unique unchangeable window ID
- `window` — raster image of this window
- `wsys` — directory of all windows, with subdirectories per winid containing their files

**pane-fs implication for pane-shell:** A pane-shell instance should expose in pane-fs:
- `body` → equivalent of rio's `text` (semantic content, not raw VT codes)
- `ctl` → equivalent of rio's `wctl` (commands)
- `attrs/cwd` → equivalent of rio's `wdir` (working directory)
- `attrs/title` → equivalent of rio's `label`

**rio `consctl` pattern:** Mode changes via file writes, with revert-on-close. This is a lease pattern — holding the file open is holding the mode. Dropping the fd reverts. pane-shell should consider this for raw mode: open a mode-control handle, close it to revert.

---

## 5. Authentication / Identity

### factotum(4) architecture

From `reference/plan9/man/4/factotum` and the security paper (`reference/plan9/papers/auth.ms`):

**factotum is a per-user file server** presenting: `rpc`, `proto`, `confirm`, `needkey`, `log`, `ctl`. Each open of `rpc` creates a new private channel.

**The RPC protocol:** `start` (select protocol + role + key template) → `read`/`write` shuttle (exchange auth data) → `authinfo` (retrieve result). The application never touches keys directly. factotum holds all secrets.

**Key selection via templates:** Templates use `attr=val` (exact match), `attr?` (must exist, any value), `attr` (must exist, null value). This is pattern-matching on key attributes, not key IDs. factotum finds the matching key.

**The `role` attribute:** Keys have `role=client` or `role=server` or `role=speakfor`. The speakfor role allows factotum to authenticate processes whose uid doesn't match factotum's — this is the delegation mechanism.

**The `confirm` file:** When a key has the `confirm` attribute, factotum refuses to use it without interactive user confirmation via the `confirm` file. A GUI (fgui) reads the confirm file, prompts the user, writes back the answer. This is consent-based security.

**The `disabled` attribute:** factotum auto-disables keys that fail authentication with `disabled=by.factotum`. Failed keys are quarantined, not retried.

**Why pane chose differently:** pane uses TLS + `.plan` + Landlock instead of a factotum-style agent because:
1. TLS handles transport authentication (certificate = key)
2. `.plan` handles authorization (what you can do)
3. Landlock handles enforcement (kernel-level)
The Transport trait provides identity uniformly across transports. This achieves factotum's goal (separate auth from services) through Rust's type system rather than a separate daemon.

**What pane should consider adopting from factotum:**
- **The `confirm` pattern:** For sensitive operations (remote pane accessing local resources), pane could use a similar consent mechanism. The `.plan` file is declarative; a `confirm`-like mechanism would be interactive.
- **Key auto-disable:** If a TLS certificate fails authentication, the PeerIdentity should be quarantined temporarily to prevent brute-force retry.
- **The `needkey` pattern:** When a required credential is missing, factotum asks an external program to provide it. pane could use a similar pattern for missing TLS certificates — prompt the user rather than failing silently.

### The auth(5) message flow

From `reference/plan9/man/5/attach`:
1. Client sends `Tauth` (afid, uname, aname) — requests auth channel
2. Server returns `Rauth` (aqid) — or Rerror if no auth required
3. Client reads/writes afid to execute auth protocol (protocol not defined by 9P)
4. Client sends `Tattach` (fid, afid, uname, aname) — presents validated afid
5. Server returns `Rattach` (qid) — connection established

**Key design point:** "That protocol's definition is not part of 9P itself." — Authentication is pluggable. 9P just provides the channel (the afid); what flows over it is between the factotums. This separation is what made it possible to add new auth protocols without changing 9P.

**pane-proto implication:** pane's session-typed handshake already separates auth from the active protocol. The handshake exchanges PeerIdentity and validates it against TLS certificates. Adding new auth methods (e.g., SSH keys, hardware tokens) should only require extending the Transport trait, not the protocol types.

---

## 6. Clipboard (/dev/snarf)

### rio(4) snarf semantics

From `reference/plan9/man/4/rio`:
- `snarf` file: "returns the string currently in the snarf buffer. Writing this file sets the contents of the snarf buffer."
- "When rio is run recursively, the inner instance uses the snarf buffer of the parent, rather than managing its own."

**Concurrency semantics:** No locking. Last writer wins. Read returns whatever was last written. There is no notification when the buffer changes — you get what's there when you read.

**Per-session, not per-window:** All windows within a rio session share one snarf buffer. This is global within the session but isolated between nested rio instances (except that inner rio delegates to parent's snarf).

**Plan 9's simplicity vs. pane's needs:**
- Plan 9: text-only, no MIME types, no metadata, no locking, no TTL
- pane: named clipboards, `ClipboardWriteLock` typestate (lock/commit/revert), `ClipboardMetadata` (sensitivity, TTL, locality), MIME negotiation

The divergence is large and justified. Plan 9's snarf was adequate for a text-centric system used by 60 researchers. pane needs to handle rich content, cross-machine transfer, and security-sensitive data.

**One insight to preserve:** rio's recursive snarf delegation is analogous to pane's `Locality::Federated` — inner instances delegate to the parent's clipboard. pane's clipboard federation should make this automatic for nested/remote sessions, with `.plan` controlling which clipboards are shared.

---

## 7. Additional Patterns Worth Considering

### The `iostats` pattern (names paper)

From `reference/plan9/papers/names.ms`: "The command encapsulates a process in a local name space, monitoring 9P requests from the process to the outside world." — iostats intercepts all file operations and reports statistics.

**pane implication:** A diagnostic pane that wraps another pane's protocol connection and logs all messages would be invaluable for debugging. This is a transparent proxy at the session layer — insert it between app and compositor. The Transport trait makes this natural: a ProxyTransport that forwards while logging.

### The exportfs -P pattern (exportfs(4))

From `reference/plan9/man/4/exportfs`: `-P patternfile` restricts exported files using regex patterns. "For a file to be exported, all lines with a prefix `+` must match and all those with prefix `-` must not match."

**pane implication:** This is exactly what `.plan` should do for pane-fs exports. When a remote observer mounts the pane namespace, `.plan` acts as the pattern file — declaring which panes and attributes are visible. The regex-based approach is simple but pane can do better with structured predicates (match on pane signature, type, sensitivity level).

### The `aan(8)` pattern (import(4))

From `reference/plan9/man/4/import`: `-p` "Push the aan(8) filter onto the connection to protect against temporary network outages."

`aan` (always-available network) is a filter that buffers messages during temporary disconnections and replays them when the connection recovers. It sits between the application and the transport, transparent to both sides.

**pane implication:** Session resumption for remote pane connections. When a TCP connection drops temporarily, rather than killing the session (SessionError::Disconnected), an aan-equivalent layer could buffer pending messages and retry the connection. This is particularly important for mobile/WiFi scenarios. Implementation: a ReconnectingTransport wrapper that buffers outgoing messages and replays on reconnection, with a configurable timeout after which it gives up.

### The `cpu` reverse-export pattern (names paper, drawterm(8))

From the names paper: "The implementation is to recreate the name space on the remote machine, using the equivalent of import to connect pieces of the terminal's name space to that of the process on the CPU server, making the terminal a file server for the CPU."

From `reference/plan9/man/8/drawterm`: drawterm "serves its local name space as well as some devices (the keyboard, mouse, and screen) to a remote CPU server, which mounts this name space on /mnt/term."

**The key insight:** The terminal exports *to* the CPU server. The terminal is a server for local devices (display, keyboard, mouse). The CPU server imports those devices and runs programs that use them. The data flows in the reverse direction from what you might expect.

**pane implication:** This maps directly to `App::connect_remote()` — a remote pane app connects to the local compositor, which imports the remote app's protocol messages. But drawterm also shows the inverse: the local machine could export its display/input devices to a remote headless instance, which then serves them to remote apps. This is the `cpu` pattern — compute remotely, display locally. pane's architecture already supports this via the unified namespace and connect_remote, but the explicit reverse-export of local devices to remote servers could be a first-class operation.

### Thread library alt (thread(2))

From `reference/plan9/man/2/thread`: The `alt` call selects among multiple channel operations (send or receive). "If at least one Alt structure can proceed, one of them is chosen at random to be executed." Terminates with CHANEND (blocking) or CHANNOBLK (non-blocking).

**Key detail:** `chanclose` prevents further sends; after close, recv returns -1 if channel is empty. This is orderly shutdown — close signals "no more data coming," receiver drains remaining items then sees the close.

**pane implication:** calloop's multi-source dispatch is the equivalent of `alt` — select among multiple event sources. The channel close semantics map to calloop source deregistration. The random selection among ready sources maps to calloop's priority-based dispatch. The C1 looper evolution (Phase 3: channel topology split) should consider calloop source priorities as the equivalent of alt's fair scheduling.

### The `label` and `wdir` files

From `reference/plan9/man/4/rio`:
- `label`: "initially contains a string with the process ID of the lead process in the window and the command being executed there. It may be written and is used as a tag when the window is hidden."
- `wdir`: "rio's idea of the current working directory of the process running in the window. The file is writable so the program may update it; rio is otherwise unaware of chdir calls its clients make."

**pane-fs implication:** These are exactly the attrs that pane-fs should expose:
- `/pane/<id>/attrs/title` — writable, used for the tag line
- `/pane/<id>/attrs/cwd` — writable, the shell/app updates it on chdir. Used for routing (plumb wdir equivalent).
