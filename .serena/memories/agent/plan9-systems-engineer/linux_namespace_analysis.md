---
type: analysis
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [linux, namespaces, mount_namespace, user_namespace, FUSE, bind_mount, 9P, v9fs, Landlock, per-pane, plan9, radical_design]
related: [architecture/kernel, architecture/fs, decision/host_as_contingent_server, reference/plan9/distribution_model, reference/plan9/divergences]
agents: [plan9-systems-engineer]
sources: [Linux kernel docs, Plan 9 port/pgrp.c, Plan 9 port/chan.c, Pike et al. 1995]
---

# Linux Namespace Analysis: Per-Pane Plan 9 Fidelity

Seven-section analysis mapping Linux namespace primitives to Plan 9's
per-process namespace model, for pane's per-pane device isolation.

## Key findings

1. **80% fidelity achievable.** Linux mount namespaces + user namespaces
   + FUSE + Landlock reproduce Plan 9's namespace model substantively.
   The 20% gap: no union directories (userspace via pane-fs), no
   per-thread namespaces (requires per-process), no kernel-native
   dynamic walk resolution (FUSE bridges this), no mount-integrated
   authentication (pre-mount TLS).

2. **Critical constraint: threads → processes.** `CLONE_NEWUSER`
   cannot differ between threads in one process. Real per-pane kernel
   namespaces require each pane to be a separate process, not thread.
   This is the single biggest architectural change.

3. **Entirely unprivileged** (inside user namespace, kernel 3.8+,
   distro-dependent sysctl). User ns + mount ns + bind + FUSE +
   Landlock + pivot_root all work without root. Only host-visible
   paths (/srv, /dev/pane/) need one-time root setup, avoidable
   via session-level user namespace.

4. **v9fs enables real Plan 9 import.** Linux kernel native 9P client
   (CONFIG_NET_9P) mounts remote 9P servers. Works in user namespaces.
   Quality concerns but functional for interop.

5. **Minimum kernel versions:**
   - Basic (FUSE, Landlock): 5.13+
   - Full per-pane namespaces: 5.13+ (5.11+ for unprivileged overlayfs)
   - Maximum fidelity (io_uring FUSE, listmount, Landlock v6): 6.14+

6. **Union bind semantics:** Replace-only is sufficient for flat device
   namespace. pane-fs handles union readdir in userspace for computed
   views. overlayfs unnecessary.

7. **Phased approach:** Phase 1 predicate model (current), Phase 2
   process-per-pane with real kernel namespaces (Pane Linux), Phase 3
   v9fs import, 9P export device, pivot_root sandboxing.

## Syscall sequence for per-pane namespace

unshare(NEWUSER|NEWNS) → write uid/gid map → make root private →
mount tmpfs for /dev/pane/ → bind-mount visible devices → bind-mount
compositor virtual devices over real → mount per-pane FUSE at /pane/ →
bind-mount /srv → (optional) pivot_root → apply Landlock.

## What Linux cannot provide

- Per-thread mount tables without CAP_SYS_ADMIN
- Plan 9's # device direct attachment
- Kernel-native union directory walk resolution
- mount(2) with integrated authentication (afd)
- Serializable namespace script (namespace(1) equivalent)
- One FUSE fd serving multiple mount points (Chan multiplexing)
- Nanosecond-scale namespace copy (Linux ~50-200us)
