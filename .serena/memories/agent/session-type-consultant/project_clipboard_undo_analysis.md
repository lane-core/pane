---
name: Clipboard + undo session-type analysis (updated 2026-03-31)
description: Deep analysis of clipboard two-interface problem (fs+protocol), security as protocol concern (TTL, access control, sensitivity), undo stack information leakage, and cross-cutting serialization argument.
type: project
---

**Updated 2026-03-31** with four-concern analysis: filesystem as protocol surface, security, undo sensitivity, two-interface problem.

**Verdict:** Conditionally sound. Six invariants required.

**1. Filesystem as protocol surface:**
- pane-fs is a protocol CLIENT (pane-fs.md line 4), not a peer. All fs writes translate to protocol messages.
- Clipboard service actor serializes all access (DLfActRiS section 2.1 actor mailbox model). No extra sync needed.
- Recommended: ctl-file pattern (read-only data, write-only ctl commands). Direct data writes are ambiguous.
- Event file is degenerate session type: mu X. Recv<Event, X>. Sound. Multiple readers = per-reader subscription.

**2. Security as protocol concern:**
- TTL does NOT violate protocol: clipboard already has Option<Data> semantics. Must emit ClipboardCleared { reason: TtlExpired } event.
- Sensitivity is metadata on write, not separate protocol step. Single commit(data, metadata) call, like ReplyPort::reply(self).
- Access control = Branch<Granted, Denied>, not error. Maps to POSIX: Granted -> success, Denied -> EACCES.
- Cross-machine "local only" is server-side read policy. Writer's protocol unchanged.

**3. Undo sensitivity:**
- RecordingOptic capturing old_value of sensitive fields = information leak.
- DynOptic needs `is_undoable() -> bool` (default true). RecordingOptic checks before recording.
- Sensitive edits are not undoable (undo stack has gap). Simpler than closure-based undo.
- attrs/undo-description must not auto-generate from sensitive property names.

**4. Two-interface problem:**
- Service serialization is sufficient for SAFETY (data consistency). Not sufficient for LIVENESS (lock timeout needed).
- Filesystem interface loses typestate ordering guarantee. Compensated by runtime enforcement at service.
- Option A (atomic fs operations) recommended over Option B (fd-scoped sessions). Simpler, covers 99% of use cases.
- Lock timeout at service level required for both protocol and filesystem clients.

**Why:** Finalizing clipboard+undo design before Tier 2 implementation.

**How to apply:** These six invariants are preconditions for clipboard implementation. The ctl-file pattern, access-as-Branch, and is_undoable() on DynOptic are load-bearing design decisions.
