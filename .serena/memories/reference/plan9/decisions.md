---
type: reference
status: current
sources: [.claude/agent-memory/plan9-systems-engineer/reference_plan9_decisions]
created: 2026-04-10
last_updated: 2026-04-10
importance: normal
keywords: [plan9, decisions, adopted, adapted, rejected, summary, pane_design_decisions]
related: [reference/plan9/_hub, reference/plan9/divergences, reference/plan9/distribution_model]
agents: [plan9-systems-engineer, pane-architect]
---

# Plan 9 design decisions for pane (short summary)

Quick reference for which Plan 9 patterns were adopted, adapted,
or rejected. For the full divergence tracker with rationale, see
`reference/plan9/divergences`.

## Adopted

- Filesystem as universal fallback (pane-fs at `/pane/`) —
  cf. `names.ms` "all resources look like file systems"
- Three-tier access model (filesystem ~30 µs / protocol ~3 µs /
  in-process sub-µs)
- User-editable routing rules (plumber pattern) — cf. `plumb(6)`
  pattern-action language
- Content transformation in routing (plumber's data-rewriting)
  — cf. `plumb.ms` `data set` / `attr add`
- Lazy application launch from routing rules (plumber
  client / start) — cf. `plumber(4)` lines 61–82
- Separation of auth from application logic (factotum principle
  via TLS) — cf. `auth.ms` section 2
- ctl-file pattern for imperative control (`proc(3)` ctl: write
  text commands) — planned for pane-fs
- Text-only file content for synthetic filesystems (`names.ms`:
  "all files contain text, not binary")
- Blocking read for change notification in filesystem layer
  (`rio(4)` wctl blocks until state change)

## Adapted

- Per-process namespaces → per-uid pane-fs views (Linux cannot
  do per-process easily)
- 9P fids (client-chosen) → proposed client-chosen PaneId
  (pending implementation)
- exportfs → pane-fs protocol bridge for remote access
  (FUSE → protocol, not 9P relay)
- cpu → reverse connection model (remote app connects back via
  TcpTransport + TLS)
- factotum → `.plan` governance + TLS client certs + Landlock
- Plumber (central server) → kit-level distributed evaluation
  (avoids `plumber` BUGS: "file name space is fixed")
- `/dev/snarf` (per-rio, no locking) → Clipboard with
  `ClipboardWriteLock` typestate (transactional, named, federated)
- rio recursive snarf delegation → Clipboard `Locality` enum
  (Local / Remote / Federated)
- `exportfs -P patternfile` (regex export filter) → `.plan`
  sandbox descriptors (Landlock)
- 9P tag multiplexing → session-typed channels (compile-time,
  not runtime tag matching)
- qid version counter → potential change-detection field in
  pane-fs stat responses

## Rejected

- 9P as wire protocol (pane-proto's typed enums are strictly
  better for a known-participant protocol)
- Union directories for namespace composition (pane-fs uses
  computed projections, no MBEFORE / MAFTER ambiguity)
- Namespace reconstruction for remote execution (unnecessary —
  protocol IS the interface)
- Auth conversation in application protocol (TLS handles this
  at transport layer)
- Central router server (single point of failure — distributed
  kit evaluation instead)
- Transparent latency (pane exposes remote state explicitly
  rather than pretending it's local)
- `srv(3)` passive service posting (no lifecycle management —
  pane-roster actively monitors via init system)
- Text-only protocol messages (pane uses Rust type system;
  text-as-interface preserved only in filesystem tier)

## Not yet evaluated

- `aan(8)` session resumption for network resilience
  (`import -p` pushes filter for outage tolerance)
- Click-based content refinement in plumb rules (`plumb.ms`:
  cursor position + regex = auto-selection)
- `/proc/N/ns` round-trippable namespace export (reconstruct
  namespace from text description)
- `consctl` close-to-revert pattern (temporary mode changes
  auto-cleanup on close)
- `wsys` cross-window directory access (`rio(4)`:
  `/dev/wsys/N/` for inter-window scripting)
