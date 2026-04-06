# Clipboard and Undo Design Analysis

Merged from Plan 9 systems engineer (filesystem design) and session-type consultant (session type analysis). 2026-03-31.

## Filesystem design decisions

1. **MIME type selection:** `data` returns best available type (text/plain default). ctl file sets preferred type per-session for sophisticated consumers.
2. **Event file:** JSONL blocking-read pattern (acme /mnt/acme/log precedent). Limit concurrent readers under standard FUSE.
3. **TTL expiry:** Return EIO on read after TTL expires, not EOF or stale data. Explicit failure.
4. **Cross-machine "local only":** Omit local-only entries from remote namespace entirely (ENOENT, invisible in readdir). Entry should not exist in remote view.
5. **Clipboard ACLs:** Unix permissions (0600 private default) + .plan namespace filtering for cross-agent sharing. No new ACL system.
6. **Undo via ctl:** `echo undo > /pane/{id}/ctl`. Observable state in attrs/ (can-undo, undo-count, undo-description). Safety is handler-level.

## Session type analysis

**Verdict:** Conditionally sound. Six invariants required.

### Filesystem as protocol surface
- pane-fs is a protocol CLIENT, not a peer. All fs writes translate to protocol messages.
- Clipboard service actor serializes all access (DLfActRiS actor mailbox model). No extra sync needed.
- ctl-file pattern recommended (read-only data, write-only ctl commands). Direct data writes are ambiguous.
- Event file is degenerate session type: mu X. Recv<Event, X>.

### Security as protocol concern
- TTL does NOT violate protocol: clipboard has Option<Data> semantics. Must emit ClipboardCleared { reason: TtlExpired } event.
- Sensitivity is metadata on write, not separate protocol step. Single commit(data, metadata) call.
- Access control = Branch<Granted, Denied>, maps to POSIX (Granted → success, Denied → EACCES).

### Undo sensitivity
- RecordingOptic capturing old_value of sensitive fields = information leak.
- DynOptic needs `is_undoable() -> bool` (default true). RecordingOptic checks before recording.
- Sensitive edits are not undoable (undo stack has gap). Simpler than closure-based undo.
- attrs/undo-description must not auto-generate from sensitive property names.

### Two-interface problem
- Service serialization sufficient for SAFETY (data consistency). Not sufficient for LIVENESS (lock timeout needed).
- Filesystem interface loses typestate ordering guarantee. Compensated by runtime enforcement at service.
- Atomic fs operations (Option A) recommended over fd-scoped sessions.
- Lock timeout at service level required for both protocol and filesystem clients.

**How to apply:** Reference when implementing clipboard and undo in pane-fs. The MIME ctl-session semantics need careful fd-level design (preferred type sticky per open fd). The is_undoable() on DynOptic and lock timeout are load-bearing.