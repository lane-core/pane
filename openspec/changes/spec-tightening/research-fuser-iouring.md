# FUSE-over-io_uring: fuser status and alternatives

## Context

The spec names `fuser` as the FUSE crate and requires FUSE-over-io_uring (Linux 6.14+) as baseline.
This research checks whether fuser can deliver that.

## Kernel interface

FUSE-over-io_uring merged in Linux 6.14 (March 2025). It is **not transparent** to userspace.
The daemon must explicitly submit `IORING_OP_URING_CMD` SQEs to the `/dev/fuse` fd using two
subcommands: `FUSE_URING_REQ_REGISTER` (register queue entries, one per CPU) and
`FUSE_URING_REQ_COMMIT_AND_FETCH` (complete a request and fetch the next one in a single
submission). The traditional read/write loop on `/dev/fuse` is still required alongside io_uring
for unsupported request types (interrupts, notifications). Per-CPU ring queues give NUMA affinity.

Source: https://docs.kernel.org/next/filesystems/fuse-io-uring.html

## Does fuser support io_uring?

**No.** fuser talks to `/dev/fuse` directly (bypassing libfuse on Linux), but uses the traditional
read/write interface. Issue [#380](https://github.com/cberner/fuser/issues/380) (Aug 2025) tracks
adding io_uring structs under the "ABI 7.42 support" milestone, but it is a checklist of kernel
structs to define — no implementation, no design discussion, zero comments.

fuser's architecture — a synchronous callback trait dispatched from a read loop — would need
significant rework to drive an io_uring submission queue.

## Does libfuse 3.18 make io_uring transparent?

**Mostly yes, for C daemons.** libfuse 3.18 (2025) handles io_uring internally: existing daemons
using `fuse_session_loop_mt` get io_uring automatically on 6.14+ kernels with no code changes.
But there are no maintained Rust bindings to libfuse 3.x. The `libfuse-sys` crate is stale
(last updated 2019, targets libfuse 2.x). Wrapping libfuse 3.18 from Rust is possible but means
taking on a C dependency and FFI surface for something pane wants to own.

## Other Rust FUSE libraries

- **fuse3** (`fuse3` crate): Async (tokio or async-io), talks to `/dev/fuse` directly. No io_uring
  support. Closer architecturally to what pane wants (async), but still uses the read/write loop.
- **fuse-rs** (zargony): Unmaintained predecessor of fuser. No io_uring.
- **easy_fuser**: Wrapper around fuser. Inherits its limitations.

No Rust FUSE library supports io_uring today.

## Reference: fuseuring (C++)

[uroni/fuseuring](https://github.com/uroni/fuseuring) is a C++ demo that drives FUSE entirely
through io_uring, bypassing libfuse. It talks to `/dev/fuse` directly, uses C++ coroutines, and
reports 643 MiB/s sequential reads. It is a proof-of-concept (forwards a single file), not a
library — but it demonstrates the `/dev/fuse` + io_uring protocol clearly and could serve as a
reference for a Rust implementation.

## Recommendation for pane

**Write a thin FUSE-over-io_uring layer directly.** Rationale:

1. No existing Rust library supports it. Waiting for fuser is betting on an unplanned feature.
2. The kernel interface is small: two io_uring subcommands, a registration protocol, and the
   standard FUSE opcodes. The `/dev/fuse` protocol is well-documented and stable.
3. pane already requires the `io-uring` crate (tokio-rs/io-uring) for other subsystems. Reuse it.
4. fuseuring (C++) provides a working reference for the io_uring submission pattern.
5. pane-fs is a translation layer with a known, bounded set of FUSE ops (read/readdir/stat on
   a synthetic filesystem). It does not need a general-purpose FUSE library.

The implementation would be: open `/dev/fuse`, mount via `fusermount3` (or direct mount syscall),
set up per-CPU io_uring rings, register queue entries, then run an async loop that processes CQEs
and resubmits via `COMMIT_AND_FETCH`. Fall back to read/write for interrupt handling.

## Spec impact

The spec should change `fuser crate` to something like `custom FUSE-over-io_uring (via io-uring
crate)` in the technology table. The pane-fs spec should note that the FUSE layer is a thin
internal module, not an external library dependency.
