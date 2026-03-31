# Clipboard Design

Cross-platform clipboard with named clipboards, lazy MIME negotiation,
declarative security policies, and filesystem projection. Designed with
input from be-systems-engineer, plan9-systems-engineer, and
session-type-consultant.

---

## Architecture

Three layers:

1. **Kit API** (pane-app) — typestate handles for transactional access.
2. **Protocol** — a separate typed channel between kit and clipboard
   service (per C1). Messages for lock/read/write/watch.
3. **Service** — a clipboard server process (or module within
   pane-headless/pane-comp). Holds named clipboard state, grants
   leases, enforces security policy. Platform backends are internal.

The clipboard is a **service, not compositor state**. Headless instances
get clipboards. The compositor integrates with the service for platform
clipboard bridging (Wayland `wl_data_device`, macOS `NSPasteboard`)
but does not own the data.

### Why not compositor-owned

- Headless instances have no compositor but need clipboards (agents
  copying data for other agents is a real use case).
- Tying clipboard to the compositor creates the architectural privilege
  the project is designed to eliminate.
- Be's clipboard was managed by app_server — a constraint, not a
  deliberate choice. Plan 9's snarf buffer being tied to rio caused
  the acme-vs-rio isolation problem.
- Clipboard federation between instances requires the clipboard to be
  independently addressable.

### BeOS lineage

BClipboard (registrar-managed, transactional Lock/Clear/Commit/Unlock,
named clipboards, MIME-typed BMessage). Key evolution: lazy writes
replace eager data dump; filesystem projection replaces messaging-only
access; security policies replace the zero-access-control model.

### Plan 9 lineage

`/dev/snarf` (flat file, read bytes / write bytes, connection-scoped).
Key evolution: MIME types and structured metadata replace raw bytes;
named clipboards replace the single-buffer model; change notification
replaces polling; security policies replace the ambient-trust model.

---

## Kit API

### Handles

```rust
/// A named clipboard. Does not hold a connection — just identifies
/// which clipboard to operate on.
pub struct Clipboard {
    name: String,
}

impl Clipboard {
    pub fn system() -> Self;                       // "system"
    pub fn named(name: &str) -> Self;              // any name
    pub fn request_write_lock(&self, messenger: &Messenger) -> Result<()>;
    pub fn read(&self, mime: &str) -> Result<Option<Vec<u8>>>;
    pub fn available_types(&self) -> Result<Vec<String>>;
    pub fn watch(&self, messenger: &Messenger) -> Result<()>;
    pub fn unwatch(&self, messenger: &Messenger) -> Result<()>;
}
```

### Write path (async lock grant)

`request_write_lock()` is non-blocking — it sends a lock request to
the clipboard service and returns immediately. The lock grant (or
denial) arrives as a message to the handler:

```rust
// Handler receives one of:
Message::ClipboardLockGranted(ClipboardWriteLock)
Message::ClipboardLockDenied { clipboard: String, reason: String }
```

`ClipboardWriteLock` is a typestate handle (C2). It is consumed by
`commit(self)` or `revert(self)`. Drop without commit = revert
(affine gap compensation, same pattern as ReplyPort).

```rust
#[must_use = "dropping without commit reverts the clipboard write"]
pub struct ClipboardWriteLock { ... }

impl ClipboardWriteLock {
    /// Commit data to the clipboard. Consumes the lock.
    pub fn commit(self, data: Vec<u8>, metadata: ClipboardMetadata);

    /// Explicitly revert. Consumes the lock.
    pub fn revert(self);
}
```

Single-call commit (data + metadata together) rather than multi-step
write sequence. The service receives one atomic message. This avoids
the protocol ordering complexity of lock → clear → write* → commit
while preserving the transactional guarantee.

### Metadata

```rust
pub struct ClipboardMetadata {
    /// MIME type of the data.
    pub content_type: String,
    /// Sensitivity and lifetime policy.
    pub sensitivity: Sensitivity,
    /// Whether this entry can be read by remote (federated) instances.
    pub locality: Locality,
}

pub enum Sensitivity {
    /// Normal clipboard data. No special handling.
    Normal,
    /// Sensitive data (passwords, tokens). Zeroized on clear,
    /// auto-cleared after TTL expires.
    Secret { ttl: Duration },
}

pub enum Locality {
    /// Readable from any instance (local or remote).
    Any,
    /// Readable only from the local instance. Remote namespaces
    /// do not see this entry (ENOENT, not empty).
    Local,
}
```

### Read path (no lock required)

Reads are non-blocking and do not require locking. The reader gets
the last committed state. This matches Be's model (Lock downloads a
snapshot) without the download-on-lock cost.

```rust
let data = clipboard.read("text/plain")?;           // specific type
let types = clipboard.available_types()?;            // what's there
```

### Change notification

```rust
clipboard.watch(&messenger)?;
// Handler receives:
Message::ClipboardChanged { name: String, source: PaneId }
// The notification carries no data — read separately (Be's model,
// avoids broadcasting large content to all watchers).
```

### Named clipboards

Globally shared by name, same API. "system" is the well-known default.
Other names are application-defined (kill-ring, registers, drag-data).
Named clipboards are not the same as editor registers — registers are
application-internal state exposed through pane attributes; named
clipboards are shared inter-application communication channels.

---

## Security

### Declarative policies via .plan

Clipboard access is governed by the same `.plan` file mechanism that
governs all pane resource access. The `.plan` declares what each
agent/user can see:

```
# .plan
clipboard/system     read write
clipboard/private    deny
```

The clipboard service checks the reader's identity against the `.plan`
at lock/read time. Denial is a protocol Branch (Granted/Denied), not
an error — maps to EACCES at the FUSE layer.

### Sensitivity and TTL

The writer declares sensitivity at commit time. The service enforces:

- **Normal**: no special handling. Data persists until overwritten.
- **Secret { ttl }**: data is zeroized (not just freed) on clear or
  TTL expiry. The service maintains a timer per entry. On expiry, the
  entry is removed and a `ClipboardCleared { reason: TtlExpired }`
  event is emitted to watchers.

TTL does not violate protocol expectations — an empty clipboard is
already a valid state. The event reason tag lets watchers distinguish
TTL expiry from explicit clear.

### Locality

Entries marked `Locality::Local` are omitted from remote namespaces
entirely. Remote `readdir` does not list them; remote `walk` gets
ENOENT. This follows the Plan 9 principle: if you should not see it,
it is not in your namespace.

### Memory hygiene

The clipboard service uses `zeroize` on drop for entries marked
`Sensitivity::Secret`. The service should avoid logging sensitive
clipboard content.

---

## Filesystem Projection

Deferred to pane-fs implementation, but the interface shape is
defined now:

```
/pane/clipboard/{name}/
    data    read-only: returns best available type (text/plain default)
    meta    read-only: JSON {types, source, timestamp, sensitivity}
    event   blocking read: JSONL change notifications
    ctl     write-only: commands (see below)
```

### data (read-only)

Returns the best available representation. Default: `text/plain` if
available, otherwise first registered type. The reader can configure
the preferred type via ctl:

```
echo "accept text/html" > /pane/clipboard/system/ctl
cat /pane/clipboard/system/data   # now returns text/html
```

The `accept` command is sticky per-fd (per open session), not global.
This avoids races between concurrent readers configuring different
types.

### ctl (write-only)

```
set <data>                     # atomic lock+write+commit (text/plain)
set-typed <mime> <data>        # atomic lock+write+commit (specific type)
clear                          # clear the clipboard
accept <mime>                  # configure data reads for this fd
```

The `set` command is the common path for scripts and editors:

```lua
-- neovim clipboard provider
vim.g.clipboard = {
  copy = {
    ['+'] = {'sh', '-c', 'printf "set %s" "$(cat)" > /pane/clipboard/system/ctl'},
    ['*'] = {'sh', '-c', 'printf "set %s" "$(cat)" > /pane/clipboard/system/ctl'},
  },
  paste = {
    ['+'] = 'cat /pane/clipboard/system/data',
    ['*'] = 'cat /pane/clipboard/system/data',
  },
}
```

The copy command writes to ctl with a `set` prefix; paste reads
from data directly.

### Error mapping

| Protocol outcome | FUSE errno |
|---|---|
| Access denied (.plan) | EACCES |
| TTL expired | EIO |
| Local-only from remote | ENOENT |
| Lock held by another | EAGAIN |

### Event file

Blocking read, one JSONL line per event. Each reader is an independent
subscription. The service fans out events to all subscribers. Limit
concurrent readers in standard FUSE mode (5 simultaneous is reasonable;
50 is not — each blocks a FUSE request slot).

Events include a reason field:
```json
{"type":"changed","name":"system","source":"<pane-id>"}
{"type":"cleared","name":"system","reason":"ttl_expired"}
{"type":"cleared","name":"system","reason":"explicit"}
```

---

## Service Architecture

### State

- Named clipboard map: `HashMap<String, ClipboardEntry>`
- Per-entry: data bytes, content type, source pane ID, timestamp,
  sensitivity, locality, TTL timer handle
- Lock state: at most one writer per named clipboard at a time
- Watcher list: per-clipboard subscriber set

### Lock management

- Write locks are leases with a configurable timeout (default 10s).
- If the lock holder disconnects or times out, the service forcibly
  reverts and releases the lock.
- Read operations do not require locking.
- Lock requests from the looper thread are refused (same as
  `send_and_wait` — would deadlock). Use the async API.

### Platform backends

The service has a platform-specific backend that bridges to the host
clipboard system:

- **Headless**: in-memory only. Named clipboards are purely pane-internal.
- **Wayland**: bridges "system" clipboard to `wl_data_device` selection.
  Lazy write via `wl_data_source` (ClipboardWriter maps naturally).
  Other named clipboards are pane-internal.
- **macOS**: bridges "system" clipboard to `NSPasteboard.general`.
  Other named clipboards are pane-internal.

Platform backends are internal to the service — the kit API is
platform-independent. MIME/UTI translation happens inside the backend.

### Federation (deferred)

Per-instance clipboards by default. Cross-instance clipboard
synchronization is an explicit opt-in configuration, not a default.
The federation protocol will share the same infrastructure as
pane-store federation when that is built. Design deferred until then.

---

## Dependencies

- **pane-fs**: for filesystem projection (deferred)
- **pane-store**: for indexable attributes on clipboard entries (deferred)
- **.plan governance**: for access control (deferred, design at
  `docs/distributed-pane.md` section 4)
- **Multi-source select in looper**: for the separate clipboard channel
  (C1, noted in `looper_message.rs` as future work)

The kit API and clipboard service can be built before these
dependencies. The filesystem projection and access control are
additive layers that compose on top.
