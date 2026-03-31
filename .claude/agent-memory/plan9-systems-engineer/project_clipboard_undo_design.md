---
name: Clipboard and undo filesystem design decisions
description: Plan 9-grounded analysis of clipboard MIME negotiation, security model, TTL, cross-machine policy, and undo via ctl — decisions made 2026-03-31
type: project
---

Clipboard filesystem design decisions (2026-03-31):

1. **MIME type selection**: Option (a) — `data` returns best available type (text/plain default). ctl file sets preferred type per-session for sophisticated consumers. Rejected separate files per MIME type (option c) as noisy/unstable.

2. **Event file**: JSONL blocking-read pattern (acme /mnt/acme/log precedent). Should limit concurrent readers under standard FUSE (not an issue with io_uring).

3. **TTL expiry behavior**: Return EIO on read after TTL expires, not EOF or stale data. Explicit failure, consistent with pane's "don't hide distribution" philosophy.

4. **Cross-machine "local only" policy**: Omit local-only entries from remote namespace entirely (ENOENT, invisible in readdir). Do not return empty or error — entry should not exist in remote view. Same filtered-view mechanism as /pane/local/ vs /pane/remote/.

5. **Clipboard ACLs**: Unix permissions (0600 private by default) + .plan namespace filtering for cross-agent sharing. No new ACL system needed.

6. **Undo via ctl**: `echo undo > /pane/{id}/ctl` is correct pattern. Observable state in attrs/ (can-undo, undo-count, undo-description). Safety is handler-level, not filesystem-level. Remote agents constrained by .plan governance.

**Why:** Lane is finalizing clipboard and undo design. These decisions follow Plan 9 idioms (ctl files, namespace-as-permission, transport-level encryption) adapted for pane's modern threat model.

**How to apply:** Reference when implementing clipboard and undo filesystem projection in pane-fs. The MIME ctl-session semantics need careful fd-level design (preferred type sticky per open fd).
