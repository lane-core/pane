# Architecture Spec Review: Dependency Philosophy Conformity

Review of `openspec/specs/architecture/spec.md` against the dependency philosophy stated in S1:

> "Pane is a radically opinionated distribution that determines its own dependencies with complete freedom. Convention and legacy do not constrain our choices. We target the latest kernel interfaces, the newest viable subsystems, and the most forward-looking infrastructure when there are significant payoffs for our design model -- provided we have reasonable confidence in their future support, or at minimum can maintain them ourselves if needed. We are futureproofing, not backward-compatible."

---

## 1. Where the Spec Is Conservative When It Should Be Bold

### 1.1 Sandboxing: seccomp + namespaces when Landlock exists

**What the spec says (S11):** "Sandboxing: seccomp + namespaces -- syscall filtering + mount/user namespace isolation."

**What the dependency philosophy implies:** Landlock (Linux 5.13+, now at ABI v6 as of kernel 6.x) is the forward-looking sandboxing primitive. It provides unprivileged filesystem and network access control without requiring CAP_SYS_ADMIN. It stacks with existing LSMs. It is actively developed (ABI v4 added TCP restrictions, v5 added ioctl restrictions, v6 added abstract Unix socket and signal scoping). The `landrun` and `island` projects demonstrate its viability as a standalone sandboxing mechanism.

seccomp-BPF is a blunt instrument -- it filters syscall numbers, not filesystem paths or network endpoints. For pane's actual sandboxing needs (restricting which files an agent can access, which sockets a pane server can connect to), Landlock is the right granularity. seccomp remains useful for reducing the syscall surface, but it should not be listed as the primary sandboxing mechanism.

**What should change:** The sandboxing entry in S11 should read "Landlock + seccomp + namespaces." Landlock for filesystem and network access control (the primary mechanism -- fine-grained, unprivileged, composable). seccomp for syscall surface reduction (defense in depth). Namespaces for mount/PID/user isolation. The agent sandboxing discussion in S4 (pane-ai) should reference Landlock explicitly -- `.plan` file permissions mapping to Landlock rulesets is a natural fit, and it's more auditable than namespace manipulation.

### 1.2 Filesystem target: btrfs-primary with XFS-alternative is hedging

**What the spec says (S3, pane-store):** "Target filesystem: btrfs (primary) or XFS (supported alternative). [...] bcachefs is a future option once it matures (2027-2028)."

**What the dependency philosophy implies:** A distribution with "complete freedom" to choose its dependencies should pick one filesystem and build for it. Supporting two filesystems means testing two code paths for xattr behavior, two sets of filesystem-specific performance characteristics, two sets of edge cases in pane-store's fanotify integration. The XFS alternative dilutes the opinion.

The bcachefs timeline needs updating: as of 2025-2026, bcachefs has been removed from the mainline kernel (ejected starting with 6.18) and is now an externally-maintained DKMS module. The "future option once it matures (2027-2028)" language is based on an outdated trajectory. bcachefs is further from "maturity" now than it was when this was written -- the governance issues that led to its removal are orthogonal to its technical quality, but they make "reasonable confidence in future support" a stretch.

**What should change:** btrfs as the sole supported installation target. Drop XFS as a "supported alternative" -- it can be mentioned as technically capable but untested. Remove the bcachefs timeline or rewrite it to acknowledge the mainline removal: "bcachefs would be a natural fit technically, but its removal from the mainline kernel in 6.18 and current status as an external DKMS module makes it unsuitable for a distribution that wants reasonable confidence in future support. We track it but do not plan around it."

### 1.3 Widget rendering: femtovg on OpenGL is not forward-looking

**What the spec says (S11):** "Widget rendering: femtovg -- 2D vector graphics on OpenGL via glow. Rounded rects, gradients, text."

**What the dependency philosophy implies:** femtovg is a NanoVG port targeting OpenGL/GLES 3.0+. It's maintained (last updated January 2026) and functional, but it's a conservative choice for a 2026 distribution. The forward-looking option is Vello (linebender), a GPU compute-centric 2D renderer built on wgpu that targets Vulkan/Metal/DX12/WebGPU. Vello has three implementations now (full GPU compute, CPU fallback, hybrid), active development from the linebender team (monthly progress reports through 2025), and is the rendering backend for Xilem.

The significance: femtovg on OpenGL means pane's widget rendering goes through GLES, which is the legacy GPU path. Modern GPU drivers optimize for Vulkan. Smithay's renderer is currently GLES-based, so there's no mismatch for compositing -- but the spec should acknowledge that OpenGL is the backward-compatible choice, not the forward-looking one, and plan for a Vulkan/wgpu path.

**What should change:** The S11 entry should acknowledge the trajectory. Either: (a) keep femtovg as the Phase 7 choice (pragmatic -- it works now, the widget rendering is not on the critical path) but note that Vello/wgpu is the target for when the rendering pipeline matures, or (b) target Vello from the start if the alpha-quality status is acceptable for Phase 7's timeline. The open question in S13 about femtovg performance should be reframed as a choice between femtovg (proven, OpenGL, conservative) and Vello (newer, wgpu/Vulkan, forward-looking).

### 1.4 The init system section offers pane-init abstraction over three backends

**What the spec says:** The research document (research-linux-stack.md S2.6) frames the init system as a three-way choice (systemd, s6, runit) with a pane-init abstraction layer mapping contracts to backends. The architecture spec (S9) commits to s6.

**What the dependency philosophy implies:** The architecture spec is correct here -- it commits to s6. But the research document's framing of "three backends" contradicts the dependency philosophy. If pane determines its own dependencies with complete freedom and chooses s6, then the pane-init abstraction layer over multiple backends is exactly the kind of backward-compatible hedging the philosophy rejects. Write for s6 directly. The abstraction buys nothing for a distribution that controls its init system.

**What should change:** The architecture spec is already doing the right thing. Ensure the research framing doesn't leak back into the spec. pane writes s6 service definitions, not "pane-init contracts mapped to backends." The boot sequence in S9 is already s6-specific and should stay that way.

---

## 2. Where the Spec Names Technologies That May Not Be the Most Forward-Looking

### 2.1 Compositor renderer: GLES when Vulkan is the future

**What the spec says (S10):** "Compositing all client buffers into the output framebuffer (via smithay's GLES renderer)."

**What the dependency philosophy implies:** Smithay's GLES2 renderer is the current default, but Vulkan rendering for Wayland compositors is actively being developed (Weston 15.0 has a Vulkan renderer). For a distribution building for the next decade, GLES is the safe conventional choice. Vulkan provides: better multi-GPU support, explicit memory management, compute shader access for effects, and alignment with where GPU drivers are optimized.

**What should change:** This is a case where "maintainable" wins over "latest." Smithay's GLES renderer is mature and well-tested. A Vulkan renderer for smithay doesn't exist in production-ready form yet. The spec should note the intent to move to Vulkan compositing when smithay supports it, but GLES is the correct Phase 4 choice. Flag it as a planned migration, not a permanent decision: "Phase 4 uses smithay's GLES renderer. When smithay's Vulkan backend matures or wgpu integration becomes viable, the compositor migrates to Vulkan for compute-shader-based effects and better multi-GPU support."

### 2.2 Wire format: postcard is fine but the rationale is thin

**What the spec says (S11):** "Wire format: postcard -- Serde-based, varint-encoded, compact binary."

**What the dependency philosophy implies:** postcard is a reasonable choice. But the spec doesn't discuss alternatives or why postcard over, say, bincode (faster, fixed-size encoding) or rkyv (zero-copy deserialization). For a system where wire format performance matters at the protocol level (1.5-3us per op target), the choice deserves more scrutiny.

**What should change:** Minor -- add a sentence on why postcard: "postcard's varint encoding minimizes wire size for the small messages typical in compositor protocols; its no_std support enables use in constrained contexts." Or, if zero-copy deserialization matters for the hot path (compositor frame batching), consider rkyv and note the tradeoff.

### 2.3 fuser crate for FUSE

**What the spec says (S11):** "FUSE: fuser crate -- `/srv/pane/` -- Plan 9-style filesystem interface."

**What the dependency philosophy implies:** The spec commits to FUSE-over-io_uring (Linux 6.14+) in S3, which is bold and correct. But this creates a question about fuser: does the fuser crate support FUSE-over-io_uring, or will pane need to use libfuse3 (which transparently supports io_uring as of 6.14) or write its own FUSE protocol handler? The io_uring FUSE support works transparently through libfuse -- "there is no need to add any specific support to the user-space server implementations: as long as the FUSE server uses libfuse, all the details are totally transparent." fuser does NOT use libfuse -- it implements the FUSE protocol directly in Rust. This means fuser would need explicit io_uring support to benefit from FUSE-over-io_uring.

**What should change:** This is a real gap. The spec commits to FUSE-over-io_uring as "not an optional optimization but a baseline expectation" while naming a FUSE crate (fuser) that implements the protocol directly and may not support io_uring. Either: (a) verify that fuser has or plans io_uring support, (b) plan to use libfuse3 bindings instead (since libfuse3 gets io_uring transparently), or (c) acknowledge that pane-fs may need to implement FUSE-over-io_uring directly. This should be an open question if not already resolved.

---

## 3. Where the Spec Should Leverage Being a Distribution

### 3.1 io_uring for more than just FUSE

**What the spec says:** io_uring is mentioned only for FUSE (S3, pane-fs) and briefly in the research as a potential optimization for pane-store's initial scan.

**What the dependency philosophy implies:** A distribution controlling its kernel version can commit to io_uring as infrastructure, not just for FUSE. Specific opportunities:

- **pane-store initial scan:** Batched `getxattr` operations via io_uring for the startup xattr scan across potentially millions of files. This is the single highest-impact use -- the research identifies it but the spec doesn't commit to it.
- **pane-fs reads/writes:** Beyond the FUSE transport, the pane-fs daemon's own I/O to backing storage can use io_uring for batching.
- **Log writes:** pane-watchdog's journal flush (currently described as "pre-opened fd, direct write(2)") could use io_uring's pre-registered buffers for guaranteed-allocation-free writes.

**What should change:** Add io_uring to S11 as a general infrastructure choice: "io_uring: FUSE transport (mandatory, Linux 6.14+), pane-store bulk I/O (startup scan), and other high-throughput file operations." Note the security consideration: io_uring has a significant CVE history (Google disabled it on servers; it accounts for ~60% of their kernel VRP submissions) and creates blind spots for security monitoring tools (Falco, etc.). For a desktop distribution this is manageable -- the attack surface is different from server environments -- but the spec should note it. The `io_uring_disabled` sysctl (Linux 6.6+) provides a kill switch if needed.

### 3.2 Landlock for agent sandboxing (detailed)

Already covered in 1.1, but the distribution angle deserves emphasis. Because pane controls its kernel, it can require Landlock ABI v6 (the latest), which provides:

- Filesystem access control (read, write, execute, make_dir, remove, etc.)
- Network access control (TCP bind, TCP connect) -- added in ABI v4 / kernel 6.7
- Abstract Unix socket scoping -- added in ABI v6
- Signal scoping -- added in ABI v6

This maps directly to `.plan` file permissions. An agent's `.plan` says "can read ~/project/, can write ~/project/output/, can connect to TCP port 443" -- this translates 1:1 to Landlock rulesets. The agent process self-sandboxes using Landlock before executing any model calls. No privilege escalation needed (Landlock is unprivileged). The sandbox is inherited by child processes and cannot be removed.

**What should change:** The pane-ai section (S4) should describe Landlock as the primary sandboxing mechanism for agents. The `.plan` -> Landlock ruleset translation is a concrete, implementable design. Seccomp remains for syscall-surface reduction; namespaces for mount isolation of the agent's filesystem view; Landlock for fine-grained access control within that view.

### 3.3 memfd_secret for sensitive agent data

**What the spec says:** memfd is mentioned for buffer sharing (S3, research). memfd_secret is not mentioned.

**What the dependency philosophy implies:** memfd_secret (Linux 5.14+) creates memory regions that are inaccessible to the kernel itself -- pages are removed from the kernel's direct map. For agent infrastructure handling API keys, model credentials, and conversation context that must not leak, this is the right primitive. It's available on any kernel pane ships (5.14 is ancient by pane's standards).

**Caveats:** memfd_secret requires the `secretmem_enable` kernel boot parameter. It prevents hibernation while secret memory is active. The uncached variant (for Spectre mitigation) has significant performance impact. These are real costs.

**What should change:** Mention memfd_secret in the pane-ai section as the mechanism for protecting sensitive agent credentials and context. Note the tradeoffs (kernel boot parameter, hibernation interaction). This is a natural fit for a distribution that controls its kernel configuration -- pane can enable `secretmem_enable` by default.

### 3.4 pidfd_getfd for compositor crash recovery

**What the spec says (S3, pane-roster):** "Process tracking uses pidfd for race-free liveness detection."

**What the dependency philosophy implies:** pidfd is used, which is good. But only `pidfd_open` and poll-on-pidfd are described. The pidfd family includes:

- `pidfd_getfd` (Linux 5.6): Duplicate a file descriptor from another process, identified by pidfd. This enables the watchdog or roster to recover file descriptors (e.g., the Wayland listening socket) from a dying compositor process and hand them to a replacement.
- `pidfd_send_signal` (Linux 5.1): Race-free signal delivery.
- `PIDFD_THREAD` flag (Linux 6.9): Thread-level pidfd, not just process-level.

The combination of pidfd_getfd + s6-fdholder means pane could potentially do zero-downtime compositor restarts: s6-fdholder holds the listening socket, the new compositor instance retrieves it, clients reconnect transparently. The spec describes this for s6-fdholder but doesn't connect pidfd_getfd to the recovery story.

**What should change:** Expand the pidfd entry in S11 to include pidfd_getfd and pidfd_send_signal. In the crash recovery discussion, note that pidfd_getfd provides a mechanism for the watchdog to inspect (or transfer) file descriptors from a process it's monitoring, which strengthens the recovery guarantees.

### 3.5 Kernel configuration as a distribution choice

**What the spec says:** The spec mentions targeting "the latest kernel interfaces" but doesn't discuss kernel configuration.

**What the dependency philosophy implies:** As a distribution, pane builds its own kernel. This means it can:

- Enable `CONFIG_SECRETMEM` (memfd_secret)
- Set `CONFIG_SECURITY_LANDLOCK=y`
- Enable `CONFIG_IO_URING=y` with appropriate security settings
- Configure `CONFIG_FANOTIFY=y` and `CONFIG_FANOTIFY_ACCESS_PERMISSIONS=y`
- Set `CONFIG_FUSE_IO_URING=y` (Linux 6.14+)
- Enable `CONFIG_BTRFS_FS=y` (not as module)
- Configure cgroup v2 only (`CONFIG_CGROUP_V1=n` if feasible)
- Set appropriate `CONFIG_DEFAULT_MMAP_MIN_ADDR` for security
- Enable `CONFIG_SCHED_CORE` for core scheduling (relevant for per-pane threads on SMT systems)

**What should change:** Add a brief section in S9 (Distribution Layer) noting that pane's kernel configuration is an opinionated distribution choice, with the key features enabled by default. This makes concrete what "we target the latest kernel interfaces" means at the kernel build level.

---

## 4. Where "Latest" Conflicts with "Maintainable"

### 4.1 io_uring security surface

**Assessment:** io_uring is a significant attack surface. Google disabled it on servers, Android, and Chrome OS. CVEs continue to be filed (CVE-2026-23259, CVE-2026-23113). Security monitoring tools (Falco, etc.) are blind to io_uring-based operations -- a rootkit demonstrated this convincingly.

**For pane specifically:** The threat model is different from Google's servers. Pane is a desktop environment, not a multi-tenant server. The io_uring usage is confined to specific subsystems (FUSE transport, pane-store bulk I/O), not exposed to arbitrary user code. The `io_uring_disabled` sysctl provides a kill switch.

**Recommendation:** Keep io_uring for FUSE (the performance benefit is real and the alternative is worse latency). Be cautious about expanding io_uring usage beyond specific, controlled subsystems. Note the security considerations in the spec. This is a case where the dependency philosophy's caveat -- "reasonable confidence in their future support" -- applies: io_uring is here to stay, but its security posture is still evolving.

### 4.2 par crate maturity

**Assessment:** The `par` crate is the foundation of pane's session type system. The spec correctly identifies the transport bridge as the highest-risk prototype (Phase 2). The par crate is by a single author (faiface) and is not widely adopted.

**For pane specifically:** This is the correct risk to take -- session types are the differentiating feature. But the spec should be honest that if par proves unmaintainable or abandons a direction pane needs, pane must be prepared to fork or reimplement. The fallback (shared types without session ordering guarantees) is described in S7, which is good.

**Recommendation:** No change needed beyond what S7 and S12 already say. The risk is acknowledged. The build sequence correctly prioritizes the transport bridge as the make-or-break prototype.

### 4.3 bcachefs -- already addressed above

The "2027-2028 future option" language is now wrong. bcachefs was removed from mainline. Update or remove.

---

## 5. Wayland Protocol Extensions: Stability Audit

The spec names specific Wayland protocol extensions in S3 (pane-comp) and S10. Here is the stability status of each as of March 2026, assessed against the dependency philosophy's preference for `ext-` (cross-compositor) over `wlr-` (wlroots-specific).

### 5.1 Protocols the spec lists

| Protocol | Stability | Namespace | Notes |
|---|---|---|---|
| xdg-shell | **Stable** | xdg | Correct choice. No issues. |
| wlr-layer-shell | **Unstable** (wlr) | wlr | No `ext-` alternative exists. Widely implemented (sway, hyprland, KDE, COSMIC, labwc). No active standardization effort in wayland-protocols. Acceptable dependency for now. |
| linux-dmabuf | **Stable** | wp | Correct choice. No issues. |
| wl_shm | **Core** | wl | Correct. |
| wl_seat | **Core** | wl | Correct. |
| xdg-decoration | **Unstable** | xdg | Still unstable in 2026. No `ext-` replacement proposed. Pane overrides it anyway (always server-side chrome), so the instability risk is low -- pane doesn't depend on the negotiation, it dictates the outcome. |
| fractional-scale | **Staging** | wp | Correct. Widely implemented, likely to stabilize. |
| viewporter | **Stable** | wp | Correct. |
| presentation-time | **Stable** | wp | Correct. |
| input method (zwp_input_method_v2) | **Unstable** | zwp | No better alternative. Essential for CJK support. The `zwp` prefix indicates it predates the current naming scheme. Accept the instability. |

### 5.2 Protocols the spec should explicitly list

| Protocol | Stability | Why pane needs it |
|---|---|---|
| ext-session-lock-v1 | **Staging** | Screen locking. Pane needs this for lock screen functionality. The spec doesn't mention screen locking at all. |
| ext-idle-notify-v1 | **Staging** | Idle detection for screen saver/power management. Not mentioned in the spec. |
| ext-image-copy-capture-v1 | **Staging** | Screen capture for WebRTC/OBS. The spec mentions PipeWire screen capture in S13 (open questions) but doesn't name the protocol. This is the standardized replacement for the deprecated wlr-screencopy. |
| ext-data-control-v1 | **Staging** | Clipboard management. Addresses the "clipboard dies when source app exits" problem. The spec discusses clipboard in the research but doesn't list this protocol. |
| wp-content-type-hint-v1 | **Staging** | Content type hints for output optimization (video, game, photo). Useful for pane-media integration. |
| ext-foreign-toplevel-list-v1 | **Staging** | Toplevel enumeration. Needed if any external tools (accessibility, task switching) need to query pane's window list. |
| ext-color-management-v1 | **Staging** | Color management / HDR. Added in wayland-protocols 1.41, actively developed through 1.47. For a futureproofing distribution, HDR support should be planned from the start. |

### 5.3 Protocols the spec uses that have ext- alternatives or should be scrutinized

| Currently listed | Better alternative | Assessment |
|---|---|---|
| wlr-output-management | No `ext-` equivalent exists | COSMIC has its own extension. No standardization effort visible. Accept wlr- for now. |
| wlr-layer-shell | No `ext-` equivalent exists | Same situation. Accept wlr- for now. |

### 5.4 What should change

The S3 pane-comp protocol list should be expanded to include the `ext-` protocols above. The spec should distinguish between:

1. **Core** (wl_*): the Wayland base. Stable, no choice to make.
2. **Stable extensions** (xdg-shell, linux-dmabuf, viewporter, presentation-time): committed.
3. **Staging `ext-` protocols** (ext-session-lock, ext-idle-notify, ext-image-copy-capture, ext-data-control, ext-color-management, fractional-scale): the forward-looking choices. These are cross-compositor and on the path to stable.
4. **wlr- protocols with no ext- alternative** (wlr-layer-shell, wlr-output-management): accepted as necessary, tracked for eventual `ext-` replacement.
5. **Unstable protocols with no alternative** (xdg-decoration, zwp_input_method_v2): accepted, low risk because pane controls the compositor behavior.

This categorization makes the dependency philosophy concrete at the protocol level: prefer `ext-` over `wlr-`, prefer stable/staging over unstable, accept wlr- only when no cross-compositor alternative exists.

---

## 6. Summary: Ranked Recommendations

### High impact, clear action

1. **Add Landlock as primary sandboxing mechanism** (S4, S11). seccomp is defense-in-depth, not the primary tool. Landlock maps directly to `.plan` permissions.
2. **Expand Wayland protocol list** (S3). Add ext-session-lock, ext-idle-notify, ext-image-copy-capture, ext-data-control, ext-color-management. Categorize by stability tier.
3. **Resolve the FUSE crate / io_uring gap** (S3). fuser implements FUSE directly and may not support FUSE-over-io_uring. This contradicts the "baseline expectation" language.
4. **Update bcachefs status** (S3). Removed from mainline kernel. Current language is based on an outdated trajectory.

### Medium impact, straightforward

5. **Drop XFS as "supported alternative"** (S3). One filesystem, one opinion. btrfs is the target.
6. **Add io_uring to S11 as general infrastructure** beyond FUSE. Include security considerations.
7. **Note the GLES -> Vulkan/wgpu migration path** (S10, S11). GLES is correct for Phase 4; the trajectory is toward Vulkan.
8. **Expand pidfd usage** (S11). Include pidfd_getfd and pidfd_send_signal in the toolbox.
9. **Add kernel configuration section** (S9). Make concrete what "latest kernel interfaces" means at the build level.

### Lower impact, worth noting

10. **Add memfd_secret for agent credential protection** (S4). Natural fit, requires kernel boot parameter.
11. **Consider Vello as femtovg successor** (S11, S13). Reframe the open question.
12. **Remove multi-backend init framing** from any residual research references. The spec correctly commits to s6; keep it clean.
13. **Add a sentence on postcard rationale** (S11). Why postcard over bincode or rkyv.

---

## Sources

- [Landlock kernel documentation](https://docs.kernel.org/userspace-api/landlock.html)
- [landrun -- Landlock sandbox tool](https://github.com/Zouuup/landrun)
- [island -- Landlock sandboxing tool](https://github.com/landlock-lsm/island)
- [FUSE-over-io_uring kernel documentation](https://www.kernel.org/doc/html/next/filesystems/fuse/fuse-io-uring.html)
- [Linux 6.14 FUSE-over-io_uring -- Phoronix](https://www.phoronix.com/news/Linux-6.14-FUSE)
- [io_uring rootkit bypasses Linux security tools -- ARMO](https://www.armosec.io/blog/io_uring-rootkit-bypasses-linux-security/)
- [bcachefs removed from mainline kernel -- LWN](https://lwn.net/Articles/1040120/)
- [bcachefs marked externally maintained -- Phoronix](https://www.phoronix.com/news/Bcachefs-Externally-Maintained)
- [Vello GPU 2D renderer](https://github.com/linebender/vello)
- [femtovg repository](https://github.com/femtovg/femtovg)
- [Wayland Protocols 1.47 -- Phoronix](https://www.phoronix.com/news/Wayland-Protocols-1.47)
- [Wayland Protocol Explorer](https://wayland.app/protocols/)
- [ext-image-copy-capture-v1 protocol](https://wayland.app/protocols/ext-image-copy-capture-v1)
- [ext-session-lock-v1 protocol](https://wayland.app/protocols/ext-session-lock-v1)
- [memfd_secret man page](https://www.man7.org/linux/man-pages//man2/memfd_secret.2.html)
- [pidfd_getfd man page](https://man7.org/linux/man-pages/man2/pidfd_getfd.2.html)
- [Weston 15.0 Vulkan renderer -- Phoronix](https://www.phoronix.com/news/Weston-15.0-Vulkan-Renderer)
