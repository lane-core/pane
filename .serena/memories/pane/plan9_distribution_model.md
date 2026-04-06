# Plan 9 Distribution Model: Remote pane-fs and cpu Mapping

From analysis of Plan 9 import/exportfs/cpu mechanisms. Covers Phase 2 design recommendations not in plan9_reference_insights.

## Remote pane-fs (import/exportfs mapping)

**Protocol bridge, not 9P mount.** `/pane/remote/<hostname>/` should NOT mount a remote FUSE filesystem. Instead, pane-fs establishes a pane protocol connection (TcpTransport + TLS) to the remote headless instance and translates FUSE operations into protocol messages. Same architecture as local pane-fs, different transport.

**Lazy connection.** Don't connect at mount time. Connect on first access to `/pane/remote/<hostname>/`. Cache the connection. Reconnect with exponential backoff on failure. This avoids Plan 9's hung-mount problem: unreachable host returns ECONNREFUSED/ETIMEDOUT, doesn't block indefinitely.

**Event forwarding.** For `/pane/remote/host/42/event`, establish a protocol subscription and translate events into JSONL lines. Events are pushed through the protocol and buffered for the reader — not polled (unlike 9P, which had no push mechanism).

**Read-only by default.** Remote state readable without special auth (subject to .plan). Remote ctl writes require explicit authorization — the .plan file controls which remote operations are permitted (analogous to exportfs -P patternfile).

**Namespace discovery.** `/pane/remote/` lists configured remote hosts from user/system config. No network auto-discovery — Plan 9's import required explicit host naming.

**Connection metadata.** `/pane/remote/<host>/status` exposes connection state, latency, last-seen timestamp. This is transparency Plan 9 didn't provide (mounts were either working or hung).

## cpu model (remote execution mapping)

**Reverse connection, not forward mount.** Remote app connects BACK to local compositor. Local compositor listens on TLS port (or tunnel via SSH). Remote app starts with `PANE_COMPOSITOR=tcp://local-machine:port`. From its perspective, it's a normal pane client — same Hello/Welcome handshake, just TCP+TLS transport instead of Unix socket.

**No namespace reconstruction.** This is where pane is simpler than Plan 9. cpu required rebuilding the entire remote namespace (/dev, /bin, /env). pane remote apps only need a compositor connection — they don't read /dev/cons (they receive Key events through the protocol). The "namespace reconstruction" problem dissolves because the protocol is the interface, not the filesystem.

**File access is separate.** If a remote app needs local files, that's SFTP/NFS/9P — not part of the pane compositor protocol. Don't conflate "run a remote pane app" with "give remote access to local files."

**Session persistence via reconnect.** When network drops between remote app and local compositor: compositor keeps pane state alive for a grace period (pane exists but no active client), client re-attaches with a session token. Add `Reconnect { session_token }` to handshake that bypasses full re-negotiation. This is what Plan 9's `recover` (Mirtchovski et al., IWP9 2006) tried at the 9P level.

**Identity forwarding.** Extend handshake with PeerIdentity: username, uid, hostname, TLS client cert fingerprint. Compositor uses this for access control and chrome display (showing pane is remote). Analogous to cpu's factotum authentication.

## Cross-cutting principles from Plan 9 distribution

1. **One interface discipline.** The enemy of composition is the number of distinct interfaces. pane has three (protocol, filesystem, kit API) — don't add a fourth. Every new feature expressible through all three tiers.
2. **Transparency of mechanism, not performance.** Same protocol locally and remotely. But expose latency, connection state, make failure explicit. Don't pretend remote is local.
3. **Failure must be part of the protocol.** A missing response is the worst error. Timeouts at transport layer, explicit "I don't know" at protocol layer. Disconnected pane emits PaneDisconnected, doesn't silently persist in layout.
4. **Convention must be enforced.** Plan 9 relied on informal convention for namespace layout — worked at Bell Labs, won't survive adversarial agents. pane enforces via .plan + Landlock, not convention.
