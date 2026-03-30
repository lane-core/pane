# FUSE Performance Research — Where the Filesystem Stops and the Protocol Begins

Research for pane spec-tightening. Pane commits to a FUSE filesystem at `/srv/pane/` as "universal FFI" — any tool, any language can participate. But FUSE introduces kernel-user round trips on every uncached operation. This document establishes where that cost matters and where it doesn't, so the architecture spec can draw a principled boundary between the filesystem interface and the typed protocol.

Sources:

- Vangoor, Tarasov, Zadok, "To FUSE or Not to FUSE: Performance of User-Space File Systems" (USENIX FAST 2017): <https://www.usenix.org/system/files/conference/fast17/fast17-vangoor.pdf>
- Vangoor, Tarasov, Zadok, "Performance and Resource Utilization of FUSE User-Space File Systems" (ACM TOS 2019): <https://dl.acm.org/doi/fullHtml/10.1145/3310148>
- Cho et al., "RFUSE: Modernizing Userspace Filesystem Framework through Scalable Kernel-Userspace Communication" (USENIX FAST 2024): <https://www.usenix.org/system/files/fast24-cho.pdf>
- FUSE kernel documentation: <https://www.kernel.org/doc/html/next/filesystems/fuse/fuse.html>
- FUSE passthrough documentation: <https://www.kernel.org/doc/html/next/filesystems/fuse-passthrough.html>
- FUSE-over-io-uring documentation: <https://docs.kernel.org/next/filesystems/fuse-io-uring.html>
- Schubert, "fuse-over-io-uring" (LWN, 2024): <https://lwn.net/Articles/997400/>
- Camandro, "FUSE over io_uring" (2025): <https://luis.camandro.org/2025-06-14-fuse-over-io_uring.html>
- FUSE and io_uring (LWN, 2023): <https://lwn.net/Articles/932079/>
- Marhubi, "Some early Linux IPC latency data" (2015): <https://kamalmarhubi.com/blog/2015/06/10/some-early-linux-ipc-latency-data/>
- virtiofs design document: <https://virtio-fs.gitlab.io/design.html>
- Pike, "Acme: A User Interface for Programmers" (1994)
- Plan 9 9P intro: <https://9p.io/sys/man/5/0intro>

---

## 1. The Cost of a FUSE Operation

### 1.1 The request path

Every uncached FUSE operation follows this path:

1. Application calls a VFS operation (read, write, stat, open, readdir, etc.)
2. The VFS layer invokes the FUSE kernel module
3. The FUSE module queues a request on `fc->pending` and sleeps the caller on `req->waitq`
4. The FUSE daemon's thread, blocked in `sys_read()` on `/dev/fuse`, wakes up
5. `fuse_dev_read()` copies the request from the kernel queue to the userspace buffer
6. The daemon processes the request (in pane-fs's case: translates to a pane protocol message, sends it to a pane server, gets a response)
7. The daemon calls `sys_write()` on `/dev/fuse` with the response
8. `fuse_dev_write()` copies the response back to the kernel request structure
9. The kernel wakes the original caller on `req->waitq`
10. The VFS operation returns

This is a minimum of **four context switches** (two kernel-user transitions for the read, two for the write) plus **two data copies** (request to userspace, response to kernel). The original calling thread sleeps for the entire round trip.

### 1.2 Measured overhead

The USENIX FAST 2017/ACM TOS 2019 study by Vangoor et al. is the most comprehensive measurement of FUSE overhead. Key findings:

**Throughput degradation:**
- Best case: imperceptible (large sequential I/O with splice optimization)
- Worst case: -83% for small I/O on SSD (4KB operations)
- Typical case: 3x slower than native ext4 for metadata-heavy workloads

**Small I/O (the case that matters for pane-fs):**
- 4KB writes: ~20% of native filesystem performance (5x slower)
- The overhead is dominated by per-request costs, not data transfer: each 4KB write triggers a `getxattr` call (for security capabilities), which doubles the traffic and doubles the kernel/user context switches

**CPU overhead:**
- Up to 31% increase in relative CPU utilization compared to ext4
- The cost is in context switches and data copies, not in actual computation

**Metadata operations:**
- Particularly expensive because they're small, frequent, and uncacheable when the data changes
- A mail server workload (metadata-heavy) showed nearly double the overhead of a file server workload (data-heavy)

**What the overhead is NOT:**
- The overhead is not in data transfer for large operations. FUSE uses splice/zero-copy for writes >1 page and reads >2 pages.
- The overhead is not in computation. The FUSE daemon's actual work is trivial for a passthrough filesystem.
- The overhead is entirely in the **per-request fixed cost**: context switches, queue management, data copies for small requests, and wakeup latency.

### 1.3 Per-request fixed cost breakdown

From the FUSE kernel documentation and RFUSE paper (FAST 2024):

| Cost component | Approximate time |
|---|---|
| Kernel → userspace context switch | ~1.5 μs (measured, pinned core) to ~3 μs (typical) |
| Userspace → kernel context switch | ~1.5 μs to ~3 μs |
| Data copy (request, small) | < 1 μs |
| Data copy (response, small) | < 1 μs |
| Queue management + wakeup | ~1-2 μs |
| **Total minimum per-request overhead** | **~7-15 μs** |

This is the floor for a single FUSE operation that hits the daemon. For pane-fs specifically, add the cost of pane-fs translating the FUSE request into a pane protocol message, sending it over a unix socket to the target server, receiving the response, and formatting it as a FUSE reply. That adds another unix socket round trip.

### 1.4 Comparison: unix socket round trip

Marhubi's IPC latency measurements (2015, one million iterations):

| Mechanism | p50 latency | p99 latency | p99.99 latency |
|---|---|---|---|
| Unix domain socket | 1,439 ns (~1.4 μs) | 1,898 ns (~1.9 μs) | 11,512 ns (~11.5 μs) |
| Pipe | 4,255 ns (~4.3 μs) | 5,352 ns (~5.4 μs) | 16,214 ns (~16.2 μs) |
| TCP loopback | 7,287 ns (~7.3 μs) | 8,573 ns (~8.6 μs) | 20,515 ns (~20.5 μs) |

A direct unix socket round trip is ~1.4 μs at p50. A FUSE operation, even for a trivial passthrough, adds ~7-15 μs of overhead *before* the daemon even begins its work. For pane-fs, the total path is:

```
Application → VFS → FUSE kernel module → pane-fs daemon (via /dev/fuse)
    → unix socket to pane-comp/pane-store/etc → response back through the whole chain
```

**Total expected latency for a pane-fs read: ~15-30 μs** (FUSE overhead + unix socket round trip to the actual server).

**Comparison with direct protocol access: ~1.5-3 μs** (just the unix socket round trip).

This means FUSE access is roughly **5-20x slower** than direct protocol access for a single operation. The ratio gets worse under contention (FUSE's single `/dev/fuse` queue serializes requests from all threads).

### 1.5 Comparison: native filesystem

Reading a file on ext4 that's in the page cache: **< 1 μs**. The VFS serves it directly from the page cache without touching the disk or any userspace process.

Reading a file on pane-fs: **~15-30 μs** every time the cache has expired, because pane-fs serves synthetic data that can change at any moment (pane state is live).

This is the fundamental tension: pane-fs serves *live state*, not *stored data*. The kernel's page cache is designed for stored data that changes infrequently. Live state either requires short cache timeouts (frequent round trips) or risks showing stale data.

---

## 2. FUSE Caching: The Mitigation

### 2.1 What FUSE caches

FUSE provides three kernel-side caches that mitigate the per-request overhead:

**Entry cache (`entry_timeout`):** Caches the result of `lookup` operations — "does this name exist in this directory, and what's its inode?" Default: 1.0 second. While cached, `ls` and path resolution don't hit the daemon.

**Attribute cache (`attr_timeout`):** Caches the result of `getattr` operations — file size, permissions, timestamps. Default: 1.0 second. While cached, `stat()` calls don't hit the daemon.

**Page cache:** Caches file content. When enabled, subsequent reads of the same file serve from the kernel page cache. Writes can be write-back cached.

**Negative lookup cache (`negative_timeout`):** Caches "this file does not exist" results. Default: 0 seconds (disabled). When enabled, repeated lookups for nonexistent files don't hit the daemon.

### 2.2 What this means for pane-fs

pane-fs serves live state. The tag line of pane 42 can change at any time because the user is editing it. This means:

- **Long cache timeouts** (seconds) risk showing stale state. `cat /srv/pane/42/tag` might return the tag line from a second ago.
- **Short cache timeouts** (milliseconds or zero) mean every access hits the daemon, paying the full ~15-30 μs cost.
- **Zero cache** (`entry_timeout=0, attr_timeout=0`) means even `ls /srv/pane/` costs a full round trip for every entry.

The right trade-off for pane-fs: **short but nonzero timeouts.** For most operations, showing state that's 100ms old is indistinguishable from showing current state. A shell script that reads `/srv/pane/42/tag` does not need microsecond freshness. A human user running `cat` certainly doesn't.

Recommended defaults for pane-fs:
- `entry_timeout`: 0.25s (directory structure changes infrequently — panes are created/destroyed at human timescales)
- `attr_timeout`: 0.1s (attributes like size change when content changes, but 100ms staleness is invisible to humans)
- Page cache: disabled for state files (tag, body, attrs), enabled for static content (if any)
- `negative_timeout`: 1.0s (nonexistent pane IDs don't appear spontaneously)

With these settings, a shell script that reads the same file twice within 100ms only pays the daemon round-trip cost once. This makes sequential `cat` operations cheap but doesn't mask rapidly changing state.

---

## 3. Access Patterns: What Works, What Doesn't

### 3.1 Patterns that work well over FUSE

**Infrequent inspection (human-speed reads):**
```sh
cat /srv/pane/42/tag          # What's the tag line of pane 42?
cat /srv/pane/42/attrs/cwd    # What directory is the shell in?
ls /srv/pane/                 # What panes exist?
cat /srv/pane/index           # Snapshot of all pane state
```
Latency: ~15-30 μs per uncached read. Irrelevant at human timescales. The user or script is processing the output for seconds or minutes between reads. This is the sweet spot for FUSE — the overhead is invisible.

**Configuration writes:**
```sh
echo "JetBrains Mono" > /srv/pane/config/comp/font
echo "delete" > /srv/pane/42/ctl
```
Latency: ~15-30 μs per write. Irrelevant — configuration changes happen at human speed. The write goes through FUSE to pane-fs to the target server. The user doesn't notice the round trip.

**Shell scripting and automation:**
```sh
# Close all panes matching a pattern
for id in $(grep -l "pattern" /srv/pane/*/tag); do
    echo "delete" > "$(dirname $id)/ctl"
done
```
This reads N tag files and writes N ctl files. With 50 panes: ~50 reads + ~matched writes. At 30 μs each, the reads take ~1.5 ms total. Shell overhead dominates. Fine.

**Event streaming (tail -f):**
```sh
tail -f /srv/pane/42/event    # Watch for events
tail -f /srv/pane/log         # Watch system log
```
FUSE supports blocking reads via the `poll` operation. The FUSE daemon implements `poll()` and calls `fuse_notify_poll()` when new data is available. The kernel wakes the blocked reader. This is exactly how acme's event file works — the file blocks until there's something to report. The overhead per event is one round trip (~15-30 μs), but events arrive at human speed (keystrokes, mouse clicks, window operations). Fine.

**The key insight: acme's event file proves this pattern works.** Acme's entire extension model — win (560 lines), mail (1,200 lines), grep integration — runs through blocking reads on filesystem files. The event file blocks until there's an event, delivers it as structured text, and the external program processes it. This is exactly what pane-fs's event files will do. The latency difference (acme used kernel-native 9P; pane uses FUSE) adds ~15-30 μs per event. At human event rates (tens of events per second, not thousands), this is negligible.

### 3.2 Patterns that DON'T work over FUSE

**High-frequency polling:**
```sh
# BAD: polling a value every 100ms in a tight loop
while true; do
    state=$(cat /srv/pane/42/attrs/focused)
    # ... react to state change ...
    sleep 0.1
done
```
At 10 reads/second: ~300 μs/second of FUSE overhead. Not terrible in isolation, but wasteful — the polling generates FUSE traffic regardless of whether the state changed. Five scripts doing this for different panes: 50 FUSE requests/second of pure overhead. This is the anti-pattern. Use the event file or pane-notify instead.

**Bulk state reads:**
```sh
# BAD: reading state of all 50 panes in a loop
for id in /srv/pane/*/; do
    tag=$(cat "$id/tag")
    body=$(cat "$id/body")
    cwd=$(cat "$id/attrs/cwd")
    focused=$(cat "$id/attrs/focused")
done
```
At 4 reads × 50 panes = 200 FUSE operations: ~3-6 ms total. Not catastrophic, but the typed protocol could do this in a single request/response (~3 μs) by asking the compositor for a bulk state snapshot. The filesystem forces per-file granularity; the protocol can batch.

**This matters when the bulk read is frequent.** A status bar updating every second, reading 4 attributes from 50 panes, generates 200 FUSE requests/second. That's ~6 ms/second of pure overhead — around 0.6% of one CPU core. Survivable, but pointless when the typed protocol can do it in microseconds.

**Low-latency paths (rendering, input):**
- Frame timing: the compositor needs to know surface state within ~1 ms for 60fps compositing. FUSE cannot participate in this path.
- Input dispatch: keystrokes must reach the focused pane within ~1-5 ms for perceived responsiveness. FUSE cannot participate.
- Drawing commands: the pane-app kit batches drawing commands and flushes them. This is async over the protocol socket, not through the filesystem.

These paths never touch pane-fs. They go through the session-typed protocol over unix sockets. The filesystem doesn't participate in anything that needs to happen within a frame time.

**Heavy concurrent access:**
FUSE's traditional architecture serializes through a single `/dev/fuse` queue. When 32 threads access pane-fs simultaneously, they contend on this queue. The FAST 2024 RFUSE paper measured FUSE's metadata throughput peaking at 2-4 concurrent processes and degrading beyond that.

For pane-fs specifically: contention is unlikely in practice (how many processes read pane state simultaneously?), but the architecture should not encourage patterns that create it. The filesystem is for scripting and inspection, not for building live dashboards with 50 concurrent readers.

### 3.3 The boundary, stated as principle

**The filesystem is for human-speed operations: inspection, scripting, configuration, event monitoring. The typed protocol is for machine-speed operations: rendering, input, high-frequency state access, bulk queries.**

More precisely:

| Access pattern | Frequency | Use filesystem? | Use protocol? |
|---|---|---|---|
| Read pane state (shell script) | Seconds between reads | Yes | — |
| Write configuration | Human-initiated | Yes | — |
| Monitor events (tail -f) | Event-driven, human rate | Yes | — |
| List all panes | Seconds between reads | Yes | — |
| Bulk state query | > 1/second | — | Yes |
| Polling for changes | Any frequency | — | Yes (use notifications) |
| Status bar updates | 1/second | — | Yes (subscribe to changes) |
| Input dispatch | Per-keystroke | — | Yes |
| Drawing/rendering | Per-frame | — | Yes |
| Frame timing | Per-frame | — | Yes |

The dividing line is roughly: **if you'd be comfortable with 30 μs of latency and per-file granularity, use the filesystem. If you need bulk access, sub-millisecond latency, or high frequency, use the protocol.**

---

## 4. What Plan 9 and Acme Teach

### 4.1 9P was kernel-native — FUSE is not

In Plan 9, the filesystem protocol (9P) was the kernel's native interface. When acme served files at `/mnt/acme/`, applications accessed them through the same kernel path as any other file. There was no extra layer. The overhead of reading `/mnt/acme/42/body` was essentially the same as reading any file — one protocol round trip to the server (acme's process), mediated by the kernel.

On Linux, FUSE adds a layer. The kernel doesn't speak the FUSE daemon's protocol natively — it speaks VFS, which the FUSE module translates into `/dev/fuse` requests, which the daemon translates into whatever it actually does. Plan 9's 9P round trip was one kernel-mediated message exchange. FUSE's round trip is two syscalls (read + write on `/dev/fuse`), four context switches, and two data copies.

This means Plan 9 could use the filesystem for everything — including real-time event monitoring — without noticeable overhead. Pane can use the filesystem for event monitoring (the overhead is small enough at human event rates) but cannot use it for anything that needs machine-speed access.

### 4.2 Acme's event file pattern validates the approach

Acme's entire extension ecosystem ran through filesystem I/O:

- `win` (terminal emulator, 560 lines): blocks on the event file, reads events, writes to body/data/addr files
- Mail reader (1,200 lines): reads mailbox state through files, writes UI through acme's filesystem
- grep integration: zero lines of code — just output format conventions

The event file blocked until there was something to report. Reading it was a blocking `read()` that returned when acme had an event. This is exactly what pane-fs's event and log files will do.

The difference: acme's event file was served over kernel-native 9P (one message round trip). Pane's event file is served over FUSE (four context switches + two data copies). The absolute cost difference is ~10-20 μs per event. At the rate events actually arrive (human interaction speed: single-digit to low tens of events per second), this difference is inaudible.

### 4.3 What 9P couldn't do that pane's typed protocol can

9P was request-response, text-oriented, and file-granularity. It could not:

- **Batch requests.** Each file access was a separate 9P transaction. Reading 50 panes' state required 50 reads.
- **Push notifications.** Clients had to poll or block-read. The server couldn't asynchronously notify a client of state changes.
- **Carry typed data.** Everything was bytes. Type information was convention, not protocol.

Pane's typed protocol (session-typed messages over unix sockets) can do all three. It can carry a `GetAllPaneState` request that returns a batch response. It can carry `Subscribe<StateChanged>` that pushes notifications. It carries Rust enums with compile-time type checking.

The filesystem and the protocol are complementary, not competing:
- The filesystem provides **universality** (any language, any tool, any script)
- The protocol provides **efficiency** (batching, push, typing) and **safety** (session types, compile-time verification)

Both access the same underlying state. The filesystem is the projection for universal access; the protocol is the native interface for performance-sensitive and type-safe access.

---

## 5. FUSE Performance Improvements: Present and Future

### 5.1 Splice / zero-copy (available now)

FUSE already uses splice for data transfer when the payload exceeds one page (writes) or two pages (reads). The splice path moves data between kernel buffers without copying to userspace. This means large reads and writes (> 4KB) don't pay the data-copy cost for the payload — only for the small request/response headers.

For pane-fs: most reads are small (a tag line is < 200 bytes, an attribute is < 1KB, the index is a few KB). Splice doesn't help for these. It would help if someone `cat`s the body of a pane with a large text buffer, but that's not the common case.

### 5.2 Passthrough mode (Linux 6.9+)

FUSE passthrough allows the daemon to associate a FUSE file with a backing file descriptor. Once associated, `read(2)`, `write(2)`, `splice(2)`, and `mmap(2)` operations bypass the daemon entirely — the kernel routes them directly to the backing file.

Performance improvement: near-native. Random reads improve ~2x, sequential writes improve ~3x, because the daemon is completely out of the loop.

**Relevance to pane-fs: limited.** Pane-fs serves synthetic state, not files backed by real files on disk. There's no backing fd to passthrough to. Passthrough is designed for overlay/container filesystems (FUSE-overlayfs), not for synthetic filesystems. However, if pane-fs ever serves actual file content (e.g., a pane body that IS a file), passthrough could eliminate FUSE overhead for that specific case.

### 5.3 FUSE-over-io-uring (Linux 6.14+)

The most significant recent improvement. Instead of the daemon reading/writing `/dev/fuse` with two separate syscalls, the daemon uses io_uring's `IORING_OP_URING_CMD` to submit request completions and fetch new requests in a single operation, through shared memory.

**Measured improvements (from the LWN article and patch benchmarks):**

| Workload | Traditional FUSE | FUSE-over-io-uring | Improvement |
|---|---|---|---|
| Paged reads (128K, 1 job) | 1,117 MB/s | 1,921 MB/s | 1.72x |
| Direct I/O reads (1024K, 4 jobs) | 3,823 MB/s | 15,022 MB/s | 3.58x |
| Memory-mapped reads (4K, 1 job) | 130 MB/s | 323 MB/s | 2.49x |
| File creates (1 thread) | 3,944 /s | 10,121 /s | 2.57x |
| File creates (4 threads) | 16,628 /s | 44,426 /s | 2.67x |

The key mechanisms:
- **50% fewer kernel-userspace transitions.** The traditional read-then-write is replaced with a single commit-and-fetch.
- **Per-CPU queues.** Each CPU core has its own io-uring queue, eliminating the single `/dev/fuse` contention point.
- **Same-core processing.** Requests are handled on the same core as the application, avoiding cache line bouncing.
- **Shared memory data transfer.** The FUSE queue ID doubles as an mmap offset, enabling zero-copy data transfer between kernel and userspace.

**Relevance to pane-fs: significant.** FUSE-over-io-uring roughly halves the per-request overhead (from ~7-15 μs to ~4-8 μs) and eliminates the concurrency bottleneck. The per-CPU queue means concurrent access scales instead of serializing. If pane-fs uses libfuse, io-uring support is transparent — no code changes needed.

This doesn't change the fundamental cost structure (FUSE is still ~3-5x slower than a direct unix socket) but it narrows the gap enough that the "human-speed" boundary moves slightly. Operations that were borderline (e.g., a status bar reading 10 attributes per second) become comfortable.

### 5.4 virtiofs (VM scenarios)

virtiofs uses FUSE as its guest-side protocol but runs the daemon on the host, communicating over virtio. The host daemon can use DAX (direct access via shared memory mapping) to eliminate data copies entirely — the guest reads file data directly from the host's page cache.

**Relevance to pane-fs: indirect.** virtiofs matters for pane's development story — if developing pane inside a VM (which the dev-vm-virtiofs change suggests), the host filesystem is accessible at near-native speed. But it doesn't help with pane-fs's synthetic filesystem performance, because there's no host file to DAX-map. virtiofs is relevant for the development workflow, not for the production FUSE filesystem.

### 5.5 Summary of improvements

| Optimization | Available since | Helps pane-fs? | How much? |
|---|---|---|---|
| Splice/zero-copy | Long available | Minimally (small payloads) | — |
| Passthrough mode | Linux 6.9 | No (synthetic fs, no backing files) | — |
| FUSE-over-io-uring | Linux 6.14 | Yes | ~2x per-request, fixes contention |
| virtiofs | Available | No (not a VM scenario) | — |
| Kernel attribute caching | Long available | Yes | Eliminates repeat reads within timeout |

The realistic performance envelope for pane-fs on a modern kernel (6.14+) with io-uring:

- **Single uncached read: ~8-20 μs** (FUSE overhead + unix socket to pane server)
- **Cached read (within attr_timeout): < 1 μs** (served from kernel cache)
- **Event delivery (blocking read wakeup): ~8-20 μs per event**
- **Concurrent access: scales with CPU cores** (per-CPU io-uring queues)

---

## 6. CUSE and Other Alternatives

### 6.1 CUSE (Character device in Userspace)

CUSE allows implementing character devices (`/dev/something`) in userspace. It's built on top of FUSE and shares the same kernel module. CUSE was originally motivated by OSS audio emulation — providing a `/dev/dsp` device implemented in userspace.

**Relevance to pane-fs: none.** CUSE provides character device semantics (open, read, write, ioctl), not filesystem semantics (directories, files, attributes). Pane-fs needs the filesystem model — panes as directories, state as files, `ls` and `cat` and shell redirection. CUSE would require a completely different interface model.

### 6.2 9P on Linux (v9fs)

Linux has a kernel-native 9P client (v9fs). A userspace 9P server could serve pane state, and the kernel would mount it like any other filesystem via v9fs. This eliminates the FUSE layer entirely — the kernel speaks 9P natively to the daemon.

**Performance characteristics of v9fs:** Recent patches (2023) improved performance ~10x for file transfers. v9fs performance is "comparable to NFS" for data operations but better for metadata due to simpler protocol. The kernel driver supports caching modes from none to write-back.

**The problem:** v9fs is designed for network filesystems (TCP, virtio transports). Using it for a local synthetic filesystem is possible but unusual. The daemon would implement the 9P server protocol, which is well-defined (13 message pairs) but lower-level than FUSE's VFS-oriented interface. Authentication, fid management, and walk semantics add complexity that FUSE handles automatically.

**The trade-off:** v9fs eliminates FUSE's `/dev/fuse` overhead but introduces 9P protocol overhead. For a local daemon, the 9P transport would be a unix socket. The per-operation cost would be approximately one unix socket round trip (~1.5 μs) plus 9P parsing, versus FUSE's ~7-15 μs. Roughly 2-5x faster.

**Assessment:** Using v9fs is architecturally interesting (Plan 9 heritage) but not worth the implementation complexity for pane's use case. FUSE-over-io-uring narrows the gap to ~2-3x, and the remaining difference is invisible at human-speed access patterns. The fuser crate provides a mature FUSE interface; there's no equivalent 9P server crate of comparable maturity. The right choice is FUSE with io-uring, not v9fs.

### 6.3 Could pane-fs bypass FUSE entirely?

In theory, pane could implement a kernel module that serves `/srv/pane/` as a native in-kernel filesystem, communicating with userspace pane servers through a custom interface. This would eliminate all FUSE overhead.

In practice: no. Kernel modules are a maintenance and security liability, the filesystem code would need to handle all VFS edge cases, and the benefit (saving ~15 μs per operation at human timescales) doesn't justify the cost. FUSE exists precisely to avoid putting application-specific filesystem code in the kernel.

---

## 7. What This Means for the Architecture Spec

### 7.1 The principle

The architecture spec currently states (§3, pane-fs):

> "The filesystem provides universality that typed protocols cannot (any language, any tool). The typed protocol provides safety that the filesystem cannot (compile-time verification, session guarantees). Both are needed. The filesystem is the universal FFI; the protocol is the verified channel."

This is correct but needs a performance dimension:

**The filesystem is for universality at human speed. The typed protocol is for performance at machine speed. The boundary between them is the per-operation cost of a FUSE round trip (~15-30 μs, or ~8-20 μs with io-uring).**

Concretely:
- Any operation that happens at human timescales (seconds between invocations) should be available through the filesystem. The FUSE overhead is invisible.
- Any operation that happens at machine timescales (per-frame, per-keystroke, multiple times per second) should use the typed protocol. The FUSE overhead is waste.
- Event streaming (tail -f patterns) works over the filesystem because events arrive at human rates, even though the delivery mechanism is FUSE. Acme proved this pattern for 30 years.

### 7.2 What pane-fs should expose

The filesystem should expose everything that scripts and tools need for **inspection, automation, and configuration:**

- Pane state (tag, body content, attributes, working directory)
- Pane control (ctl file for commands)
- Event streams (blocking-read files for lifecycle events, input events)
- Configuration (mirroring `/etc/pane/` with live read/write)
- System index (list of all panes with summary state)
- Routing (plumb ports for sending/receiving routed messages)

The filesystem should NOT be used for:

- Status bars or dashboards that poll frequently — use protocol subscriptions
- Anything in the rendering path — use the protocol
- Anything in the input path — use the protocol
- Bulk state queries that repeat more than ~once per second — use the protocol

### 7.3 What the spec should say about pane-fs threading

The architecture spec currently says pane-fs uses a "thread pool (FUSE operations may block)." This is correct. The sizing consideration:

With FUSE-over-io-uring (Linux 6.14+), the kernel maintains per-CPU queues and the thread pool should have at least one thread per CPU core. Without io-uring, libfuse's default threading (start with one thread, spawn more when the pending queue exceeds 2 requests, idle threads exit when pool exceeds 10) is adequate for pane-fs's expected load.

The thread pool is capped by practical concurrency — pane-fs serves a desktop, not a data center. 10-20 concurrent FUSE requests would be an extreme load (it would mean 10-20 separate processes simultaneously accessing pane state). The default libfuse threading is fine.

### 7.4 Tiered access model

Rather than a binary filesystem/protocol split, the architecture can describe three tiers:

**Tier 1: Filesystem (universal, human-speed)**
- Any language, any tool
- ~15-30 μs per uncached operation (improving to ~8-20 μs with io-uring)
- Per-file granularity
- Blocking reads for event streams
- Appropriate for: shell scripts, one-off inspection, configuration writes, event monitoring, automation

**Tier 2: Protocol (typed, machine-speed)**
- Rust (pane-app kit) or any language with a protocol client
- ~1.5-3 μs per operation (unix socket round trip)
- Batch-capable, push-capable
- Session-typed for compile-time safety
- Appropriate for: native pane applications, status widgets, live dashboards, high-frequency queries

**Tier 3: In-process (kit, zero-copy)**
- Rust only (pane-app kit, pane-ui kit)
- Sub-microsecond (function call + channel send)
- Used for: rendering, input dispatch, frame timing — anything on the compositor's hot path

The tiers compose cleanly: a shell script uses Tier 1, a pane-native status bar uses Tier 2, the compositor's rendering loop uses Tier 3. There's no cliff between tiers — the same state is accessible through all three, at different performance points.

---

## 8. Synthesis: How FUSE Performance Informs Pane's Design

### The cost is real but bounded

FUSE adds ~15-30 μs per operation (improving to ~8-20 μs with io-uring). This is 5-20x slower than a direct unix socket round trip. The cost is in fixed per-request overhead (context switches, data copies, queue management), not in data transfer.

### The cost is invisible for the intended use case

Pane-fs is designed for shell scripts, inspection, configuration, and event monitoring. These are human-speed operations. The FUSE overhead is undetectable at human timescales. Acme proved that a filesystem-based extension model works — with 9P's lower overhead, but the pattern holds even with FUSE's higher overhead, because the events arrive at human speed.

### The cost matters for the wrong use cases

Polling, bulk state reads, anything in a rendering path, anything that happens per-frame or per-keystroke — these should not go through FUSE. The architecture already has the right answer for these: the typed protocol over unix sockets, with session types for safety and batching for efficiency.

### The key decision FUSE performance validates

The architecture's commitment to two interfaces (filesystem + typed protocol) is not a compromise — it's the right design. A single interface can't serve both universality and performance. The filesystem provides one; the protocol provides the other. FUSE performance research confirms that the boundary between them is clear and principled: human-speed access goes through the filesystem, machine-speed access goes through the protocol.

### io-uring is worth targeting

FUSE-over-io-uring (Linux 6.14+) roughly halves the per-request cost and eliminates the concurrency bottleneck. Since pane is a distribution (it controls the kernel version), it can target 6.14+ and get these benefits for free. The fuser crate will need io-uring support, but this is transparent to pane-fs's code.

### 9P nostalgia is not actionable

Using v9fs instead of FUSE would be ~2-5x faster per operation, which would be meaningful at machine speed but invisible at human speed. The implementation cost (9P server in Rust, less mature ecosystem) doesn't justify the savings. FUSE with io-uring is the pragmatic choice.
