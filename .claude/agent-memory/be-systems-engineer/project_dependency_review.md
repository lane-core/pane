---
name: Dependency philosophy review findings (2026-03-20)
description: Key findings from reviewing architecture spec against dependency philosophy -- Landlock missing, bcachefs status wrong, fuser/io_uring gap, Wayland protocol audit
type: project
---

Review of architecture spec against S1 dependency philosophy. Written to `openspec/changes/spec-tightening/review-architecture-dependency-philosophy.md`.

Key findings:

1. **Landlock absent from sandboxing** -- spec says seccomp+namespaces, but Landlock (ABI v6, unprivileged, filesystem+network+signal scoping) is the right primary mechanism. Maps directly to .plan permissions for agent sandboxing.

2. **bcachefs status outdated** -- spec says "future option (2027-2028)" but bcachefs was removed from mainline kernel in 6.18 (2025), now external DKMS module. Language needs updating.

3. **fuser crate vs FUSE-over-io_uring gap** -- spec commits to FUSE-over-io_uring as "baseline expectation" but fuser implements FUSE protocol directly (not through libfuse), so it may not get io_uring support transparently. Unresolved.

4. **Wayland protocol audit** -- spec lists ~10 protocols, missing ext-session-lock, ext-idle-notify, ext-image-copy-capture, ext-data-control, ext-color-management. All staging ext- protocols that align with futureproofing philosophy.

5. **femtovg on OpenGL is conservative** -- Vello (wgpu/Vulkan) is the forward-looking choice but still alpha. Acceptable for Phase 7 but should note migration trajectory.

6. **memfd_secret not mentioned** for agent credential protection. Natural fit, requires kernel boot param.

**Why:** The dependency philosophy in S1 is strong but the rest of the spec doesn't fully follow through. Several choices are conventional rather than forward-looking.

**How to apply:** These findings inform the spec rewrite. High-priority items: Landlock, Wayland protocol expansion, bcachefs correction, fuser/io_uring resolution.
