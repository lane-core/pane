---
name: key Plan 9 design decisions for pane
description: Quick reference of which Plan 9 patterns were adopted, adapted, or rejected for pane and why
type: reference
---

## Adopted
- Filesystem as universal fallback (pane-fs at /pane/)
- Three-tier access model (filesystem ~30us / protocol ~3us / in-process sub-us)
- User-editable routing rules (plumber pattern)
- Content transformation in routing (plumber's data-rewriting)
- Lazy application launch from routing rules (plumber client)
- Separation of auth from application logic (factotum principle via TLS)

## Adapted
- Per-process namespaces -> per-uid pane-fs views (Linux cannot do per-process easily)
- 9P fids (client-chosen) -> proposed client-chosen PaneId (pending implementation)
- exportfs -> pane-fs protocol bridge for remote access (FUSE->protocol, not 9P relay)
- cpu -> reverse connection model (remote app connects back via TcpTransport+TLS)
- factotum -> .plan governance + TLS client certs + Landlock
- Plumber (central server) -> kit-level distributed evaluation

## Rejected
- 9P as wire protocol (pane-proto's typed enums are strictly better for a known-participant protocol)
- Union directories (ambiguity not worth the power for pane's use case)
- Namespace reconstruction for remote execution (unnecessary — protocol IS the interface)
- Auth conversation in application protocol (TLS handles this at transport layer)
- Central router server (single point of failure — distributed kit evaluation instead)
- Transparent latency (pane exposes remote state explicitly rather than pretending it's local)
