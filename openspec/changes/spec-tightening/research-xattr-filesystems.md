# Extended Attribute Support Across Linux Filesystems

Research for pane spec-tightening. Context: pane-store needs xattrs as a metadata layer — the Linux equivalent of BFS attributes. Pane writes typed metadata to files as `user.pane.*` xattrs, indexes them in userspace, and provides BQuery-style queries. The architecture spec already states "ext4 is not supported as an installation target" — this research validates that decision and evaluates the alternatives.

Primary sources: Linux kernel source (v6.12), kernel documentation, man pages, filesystem-specific documentation, OpenZFS documentation.

---

## VFS-Level Limits

The Linux kernel defines xattr limits in `include/uapi/linux/limits.h`:

- **XATTR_NAME_MAX**: 255 bytes (attribute name including namespace prefix)
- **XATTR_SIZE_MAX**: 65,536 bytes (64KB, maximum single attribute value)
- **XATTR_LIST_MAX**: 65,536 bytes (64KB, maximum total size of all attribute names on one inode)

These are VFS ceilings. Individual filesystems may impose tighter limits. The `user.*` namespace is available to unprivileged processes on regular files (not on symlinks or special files), governed by standard file permissions.

---

## Filesystem Comparison

### btrfs

**Per-value limit:** ~16,228 bytes with default 16KB nodesize. The limit is computed as `nodesize - sizeof(btrfs_header) - sizeof(btrfs_item) - sizeof(btrfs_dir_item)` = 16384 - 101 - 25 - 30 = 16,228. With 64KB nodesize: ~65,380 bytes (effectively the VFS maximum).

**Per-inode total:** No fixed total limit. Each xattr is a separate item in the B-tree. The filesystem can store an arbitrary number of xattrs per inode — they are individual items keyed by (inode, xattr name), not packed into a fixed-size block. The practical limit is metadata space on the volume.

**Storage mechanism:** Xattrs are items in the filesystem B-tree, stored inline in leaf nodes alongside other metadata. Small xattrs benefit from copy-on-write and checksumming automatically. There is no separate "xattr block" concept — the B-tree is the storage.

**Performance:** B-tree lookup by (inode, name) is O(log n). Reading all xattrs for an inode requires iterating items with that inode's key prefix. For pane's use case (5-15 xattrs per file, each under 1KB), this is well within btrfs's design point. Btrfs handles millions of metadata items efficiently — it was designed for this workload.

**The nodesize question:** The default 16KB nodesize limits individual xattr values to ~16KB. This is sufficient for pane's metadata (MIME types, status strings, numeric attributes, short descriptions). If a future use case requires larger attribute values (e.g., embedded thumbnails, which BFS never stored as attributes anyway), the filesystem can be formatted with 64KB nodesize, but this increases memory pressure for metadata operations. The recommendation: use default 16KB nodesize and keep individual attribute values under 4KB. If an attribute value is large enough to hit the 16KB ceiling, it should probably be a file, not an attribute.

**Verdict: Primary target.** btrfs is the default filesystem for Fedora, openSUSE, and several other major distributions. It has mature xattr support, no practical per-inode total limit, checksumming, snapshots, and a decade of production hardening. The 16KB per-value limit with default nodesize is not a constraint for pane's metadata use case.

---

### XFS

**Per-value limit:** 64KB (ATTR_MAX_VALUELEN = 64*1024). This matches the VFS maximum.

**Per-inode total:** No fixed total limit. XFS uses a tiered attribute storage system that scales from a handful of small attributes to thousands of large ones.

**Storage mechanism:** Three-tier architecture:

1. **Shortform:** When total attribute data is small enough to fit in the inode's attribute fork (alongside the data fork), attributes are stored inline in the inode itself. Shortform value lengths are stored as `__u8`, so individual shortform values max at 255 bytes — but this is just the inline fast path.
2. **Leaf:** When attributes outgrow shortform, they move to a dedicated leaf block in the attribute fork. Names are always stored in the leaf block. Values under ~75% of the filesystem block size are colocated in the leaf.
3. **Node (B-tree):** When attributes outgrow a single leaf, XFS builds a directory/attribute B-tree (dabtree) that maps hashed attribute names to leaf blocks. Values exceeding the leaf threshold are stored in "remote value blocks" — dedicated extents in the attribute fork.

This tiered system means XFS gracefully handles everything from a file with one small xattr to a file with hundreds of 64KB xattrs. The attribute fork grows independently of the data fork.

**Performance:** XFS's attribute architecture was designed for heavy metadata workloads — it originated at SGI for IRIX, where extended attributes were used extensively for DMAPI (data management) and ACLs on millions of files. The dabtree provides O(log n) lookup. Shortform attributes (the common case for pane: a few small attributes per file) are read in the same I/O as the inode itself — zero extra seeks.

**The attr2 format:** Modern XFS (on-disk format v5, which is the only format `mkfs.xfs` creates since xfsprogs 4.2) uses the attr2 inode format, which dynamically partitions the inode between data and attribute forks. This is the default and the only relevant behavior — older formats are not a concern.

**Verdict: Strong alternative.** XFS has the most mature and capable xattr implementation of any Linux filesystem. 64KB per-value limit, no per-inode total limit, inline storage for small attributes, B-tree scaling for large attribute sets. XFS is the default on RHEL/CentOS/Rocky. The tradeoff vs. btrfs: XFS lacks snapshots and checksumming (scrub catches silent corruption but doesn't prevent it). For pane's metadata use case, XFS is technically superior for xattr performance; btrfs is better as a general-purpose desktop filesystem.

---

### ext4

**Per-value limit:** Nominally one filesystem block minus xattr header overhead. With default 4KB blocks: approximately 4,000 bytes for a single value. But there's a subtlety.

**Per-inode total:** All xattrs on a single inode must fit in two locations:
1. **In-inode space:** Between the end of the inode entry (128 bytes + i_extra_isize) and the end of the inode. With default 256-byte inodes and i_extra_isize=28: 256 - 128 - 28 = **100 bytes** for inline xattrs. With 512-byte inodes: 356 bytes.
2. **One external block:** A single additional block (referenced by `inode.i_file_acl`) storing xattrs with a 32-byte header. With 4KB blocks: ~4,064 bytes.

Total budget with defaults: roughly **4,164 bytes** for all xattr names, values, and per-entry overhead on one file. This is the hard wall.

**ea_inode feature:** Linux 4.13 added the `INCOMPAT_EA_INODE` feature, which allows individual large xattr values to be stored in dedicated inodes. This lifts the per-value size limit to the VFS maximum (64KB). However: ea_inode does not increase the per-inode *total* xattr budget — it allows individual values to be large, but the index entries (name + pointer) still must fit in the in-inode space plus one external block. And ea_inode is not universally enabled — it requires `mkfs.ext4 -O ea_inode` or must be set in the superblock after creation.

**Why ext4 is insufficient for pane:** A file with pane metadata might carry: `user.pane.type` (string, ~20 bytes), `user.pane.mime` (string, ~30 bytes), `user.pane.description` (string, ~200 bytes), `user.pane.status` (string, ~10 bytes), `user.pane.tags` (string, ~100 bytes), `user.pane.created` (int64, 8 bytes), plus per-entry overhead (~32 bytes each: 16-byte aligned name + 4-byte name_index + 4-byte value offset + 4-byte value size). Total: roughly 600 bytes for a modest set of attributes. This fits — barely. But the moment applications (email, music, contacts) start writing richer attributes — as they did on BFS, and as pane is designed to encourage — the 4KB wall becomes a hard ceiling that prevents the ecosystem from growing. BFS files routinely carried 2-4KB of attribute data for email alone. The 4KB total budget, shared with ACL xattrs and security labels, is not enough headroom.

**Verdict: Not supported.** The architecture spec's decision stands. ext4's xattr budget is fundamentally incompatible with the filesystem-as-database vision. This is not a minor inconvenience — it's a structural impossibility. You cannot build BQuery-style infrastructure on a filesystem that limits total metadata per file to 4KB.

---

### bcachefs

**Per-value limit:** Determined by the btree key value size. The `x_val_len` field in `struct bch_xattr` is `__le16` (16-bit), giving a theoretical maximum of 65,535 bytes per value. The practical limit depends on btree node size and the constraint in `bch2_xattr_set()`: `if (u64s > U8_MAX) return -ERANGE`, where `u64s` is the total key size in 8-byte units. This limits a single xattr entry (name + value + overhead) to 255 * 8 = 2,040 bytes. For larger values, bcachefs would need to use a different storage path (indirect/extent-based), which as of kernel 6.12 does not appear to be implemented for xattrs.

**Per-inode total:** No fixed total limit. Like btrfs, xattrs are btree entries keyed by (inode, hash). The number of xattrs per inode is limited only by metadata space.

**Storage mechanism:** Xattrs are stored as entries in a dedicated xattr btree (`BTREE_ID_xattrs`). Names are hashed for lookup using the filesystem's hash function. The btree is copy-on-write with checksumming. The design is clean — essentially a key-value store with the inode number as the primary key.

**Maturity concern:** bcachefs was merged into Linux 6.7 (December 2023). As of early 2026, it has seen roughly two years of mainline development. Users report running it on 100TB+ filesystems. However, the project's own documentation notes ongoing work on online fsck, and as of kernel 6.18 bcachefs ships as a DKMS module rather than in-tree, suggesting ongoing tension with the kernel development process. The xattr implementation follows standard VFS patterns and is functional, but bcachefs has not seen the decade of xattr-heavy production workloads that btrfs and XFS have.

**The ~2KB per-value constraint:** This is the most significant finding. bcachefs's current xattr implementation appears to limit individual values to ~2KB due to the btree key size constraint (`u64s` stored as `u8`). This is tighter than btrfs (16KB) and XFS (64KB). For pane's typical metadata (short strings, integers), 2KB per value is sufficient. But it leaves less headroom than the other options.

**Verdict: Future option, not primary target.** bcachefs has the right architecture (btree-based, COW, checksummed), but the combination of maturity concerns, the DKMS distribution model, and the ~2KB per-value xattr constraint make it premature as a primary target. Monitor for the 2027-2028 timeframe. If bcachefs stabilizes its kernel relationship and lifts the per-value limit (which the `__le16` x_val_len field already supports — the constraint is in the btree key size), it becomes a strong candidate.

---

### f2fs

**Per-value limit:** Determined by `MAX_VALUE_LEN(inode)`, which depends on the available xattr space after accounting for the xattr header and entry overhead. In practice, with default settings: roughly 3,400 bytes per value using inline xattr storage.

**Per-inode total:** Two-tier budget:
1. **Inline xattrs:** Stored in the inode block itself. Limited to ~3.4KB (configurable via `inline_xattr_size` mount option).
2. **External xattr block:** An additional node block referenced by `i_xattr_nid`. Size: PAGE_SIZE minus node footer overhead (~4,000 bytes on 4KB-page systems).

Total budget: ~7.4KB with both tiers. Better than ext4, but still finite and small.

**Storage mechanism:** Inline xattrs are in the inode structure (fast, no extra I/O). When inline space is exhausted, f2fs allocates a dedicated xattr node block. There is no B-tree or dynamic growth beyond these two tiers — the total xattr space per inode is fixed.

**Design context:** f2fs was designed by Samsung for NAND flash (Android phones, SSDs). Its xattr support is functional but modest — Android uses xattrs primarily for SELinux labels (one small attribute per file), not for rich metadata. The filesystem's design priorities are flash-friendly write patterns, not metadata-heavy workloads.

**Verdict: Not recommended.** f2fs's xattr budget (~7.4KB total) is better than ext4 but still too constrained for the filesystem-as-database model. More importantly, f2fs's design priorities (flash wear leveling, Android use cases) do not align with pane's desktop metadata workload. f2fs is a fine filesystem for an SSD — but pane should use btrfs or XFS on SSDs, both of which are SSD-aware (btrfs SSD mode, XFS with discard support).

---

### ZFS on Linux (OpenZFS)

**Per-value limit:** In SA (system attribute) mode: 64KB stored in the dnode's bonus buffer and spill blocks. In dir mode: no practical limit (each xattr is a file in a hidden directory).

**Per-inode total:** In SA mode: up to 64KB total in system attribute space, with automatic fallback to dir mode for overflow. In dir mode: no practical limit.

**Storage mechanism — the two modes:**

1. **xattr=sa (system attribute, default since OpenZFS 0.6.5):** Xattrs are stored inline in the dnode alongside other file metadata. Fast — reading xattrs doesn't require additional I/O beyond reading the dnode. Recommended setting. The `dnodesize=auto` property allows ZFS to allocate larger dnodes when xattr data exceeds the default 512-byte dnode, up to 16KB per dnode. When SA space is exhausted, ZFS automatically falls back to dir mode for overflow attributes.

2. **xattr=dir (directory mode):** Each xattr is stored as a file in a hidden directory associated with the file. No practical size or count limit, but each xattr read/write requires a full directory lookup + file read/write — orders of magnitude slower than SA mode. This was the original Solaris implementation.

**Performance:** SA mode performance is excellent for the common case (a few small xattrs per file). The automatic fallback to dir mode means large/numerous xattrs still work, just slower. The pathological case: many files each with xattrs that *just barely* exceed SA capacity, forcing widespread dir-mode fallback. With `dnodesize=auto`, this threshold is much higher.

**Licensing concern:** OpenZFS uses CDDL, which is GPL-incompatible. ZFS on Linux operates as a DKMS out-of-tree module. This creates a distribution and support burden: the pane distribution would need to build and maintain ZFS DKMS packages, handle kernel version compatibility, and accept that ZFS cannot be part of the Linux kernel proper. This is a real ongoing cost.

**Verdict: Functional but not recommended as primary target.** ZFS's xattr support (with `xattr=sa` and `dnodesize=auto`) is technically adequate. But the CDDL licensing means ZFS will never be in-tree, creating a permanent maintenance burden for any distribution that ships it. If users want to install pane on a ZFS root, pane-store should work — but pane should not require or default to ZFS.

---

## Filesystem-Level xattr Indexing

**BFS had it.** BFS maintained B+ tree indices over attribute values at the filesystem level. `fs_create_index("MAIL:subject")` told BFS to build a global index for all files' `MAIL:subject` attributes. BQuery evaluated predicates against these indices in O(log n) time.

**Linux does not have it.** No Linux filesystem provides kernel-level xattr indexing. There is no equivalent of `fs_create_index()`. The VFS xattr interface is purely per-file: get, set, list, remove. There is no `query_xattr()`, no index management, no predicate evaluation.

This is a fundamental architectural gap. It means:

- **Querying xattrs across files requires a full scan.** To find all files where `user.pane.type == "email"`, you must enumerate files, read each file's xattrs, and filter. On a volume with millions of files, this is O(n) per query.
- **Indexing must be userspace.** pane-store builds and maintains its own in-memory index, rebuilt from xattr scans at startup. This is the correct approach given Linux's constraints.
- **Change detection substitutes for kernel-level index maintenance.** Instead of the filesystem updating its index on every attribute write (as BFS did), pane-store uses fanotify to detect attribute changes and updates its userspace index reactively.

**Has anyone proposed kernel-level xattr indexing?** No serious proposal has been merged or accepted. The closest things:

- **Tracker/Baloo/recoll-style desktop search:** These index file *content* and some metadata, but they are application-level databases, not filesystem features. They use inotify for change detection and maintain SQLite/Xapian databases. They do not provide BQuery-style live queries.
- **btrfs send/receive metadata:** btrfs can stream filesystem changes, but this is for replication, not querying.
- **fsverity/IMA:** These use xattrs for integrity, not for querying.

The absence of kernel-level xattr indexing is unlikely to change. It would require filesystem-specific implementations (each filesystem stores xattrs differently), a new VFS interface for index management and query evaluation, and consensus that the kernel should provide database-like functionality — which contradicts the Unix philosophy of simple kernel primitives + userspace policy. Pane-store's userspace indexing approach is the right design for Linux.

---

## fanotify FAN_ATTRIB for xattr Change Detection

**Does it work?** Yes. The VFS layer calls `fsnotify_xattr(dentry)` after every successful `setxattr()` and `removexattr()` operation (in `__vfs_setxattr_noperm` and `__vfs_removexattr_locked` in `fs/xattr.c`). `fsnotify_xattr()` emits `FS_ATTRIB`, which maps to `FAN_ATTRIB` for fanotify and `IN_ATTRIB` for inotify. This happens at the VFS level, *before* control returns to the filesystem — so it works uniformly across all filesystems.

**Kernel version requirements:** FAN_ATTRIB was added in Linux 5.1. FAN_MARK_FILESYSTEM (which enables mount-wide monitoring with a single mark) was added in Linux 4.20. FAN_REPORT_FID (which provides file identification via file handles, required for FAN_ATTRIB) was also added in Linux 5.1. Pane requires kernel 5.1+ — reasonable for a 2026 distribution.

**What FAN_ATTRIB catches:**
- `setxattr()` and `removexattr()` — yes, via `fsnotify_xattr()`
- `chmod()`, `chown()`, `utimes()` — yes, these also emit `FS_ATTRIB` via `notify_change()`
- Truncation — yes

**What FAN_ATTRIB does NOT distinguish:** The event does not tell you *which* xattr changed or *what kind* of metadata change occurred. pane-store receives "metadata changed on inode X" and must re-read the `user.pane.*` xattrs to determine what changed. This is a minor inefficiency: most FAN_ATTRIB events on a running system are permission changes and timestamp updates, not xattr changes. pane-store's event handler should read the file's `user.pane.*` xattrs, diff against its cached values, and update the index only if pane-relevant attributes changed.

**Filesystem uniformity:** Because `fsnotify_xattr()` is in the VFS layer (`fs/xattr.c`), it works identically on btrfs, XFS, ext4, f2fs, bcachefs, and ZFS. Filesystems that bypass the VFS xattr path (e.g., a filesystem implementing xattrs entirely in its own code without calling `__vfs_setxattr_noperm`) would not emit the event — but all major Linux filesystems use the standard VFS path.

**FAN_MARK_FILESYSTEM scope:** With `FAN_MARK_FILESYSTEM`, one fanotify mark covers the entire filesystem mount. pane-store needs one mark per filesystem where `user.pane.*` xattrs are used — typically one or two (root filesystem + home, or just root if home is on the same filesystem). The concern noted in the architecture spec ("on a system with millions of files, how much event traffic does this generate?") is addressable: the event rate is proportional to the rate of metadata changes, not the number of files. On a typical desktop, metadata changes happen at human-interaction speed (file saves, email arrivals, tag edits) — tens per second at most, not thousands.

---

## Recommendations for Pane

### Primary target: btrfs
- Default filesystem for multiple major distributions
- No per-inode total xattr limit (B-tree storage)
- ~16KB per-value limit with default nodesize (sufficient for all pane metadata)
- Copy-on-write, checksumming, snapshots included
- Decade of production hardening
- SSD-aware (SSD mount option, TRIM/discard support)

### Supported alternative: XFS
- Technically superior xattr implementation (64KB per value, tiered storage)
- Default on RHEL ecosystem
- No per-inode total limit (dabtree scales arbitrarily)
- Inline shortform xattrs for the common case (fastest possible reads)
- Lacks snapshots; no checksumming of data (metadata-only CRC32)

### Not supported: ext4
- ~4KB total xattr budget per inode is structurally insufficient
- ea_inode feature helps per-value size but not total budget
- Not a pane installation target

### Not recommended: f2fs
- ~7.4KB total xattr budget per inode — better than ext4, still constrained
- Design optimized for Android/flash, not desktop metadata workloads
- No growth path beyond two-tier xattr storage

### Future option: bcachefs
- Right architecture (btree-based COW with checksums)
- Current ~2KB per-value limit in btree key storage is a constraint
- Maturity and kernel distribution status need to stabilize
- Re-evaluate in 2027-2028

### Works but not recommended: ZFS on Linux
- xattr=sa with dnodesize=auto provides adequate xattr support
- CDDL licensing prevents in-tree kernel integration
- Permanent DKMS maintenance burden for any distribution shipping it
- If users install pane on ZFS, pane-store should work — but pane should not default to or require ZFS

### Userspace indexing is the correct approach
- No Linux filesystem provides BFS-style kernel-level xattr indexing
- No serious proposals to add it exist
- pane-store's fanotify-based change detection + userspace index is the right design
- fanotify FAN_ATTRIB works uniformly across all filesystems via the VFS layer

### Architecture spec correction needed
The architecture spec (line 164) states "btrfs and XFS both support 64KB per xattr value." This is inaccurate for btrfs: the per-value limit is ~16KB with the default 16KB nodesize, not 64KB. The 64KB figure applies only if the filesystem is formatted with `mkfs.btrfs -n 65536`. The spec should say: "XFS supports 64KB per xattr value; btrfs supports ~16KB with default settings (sufficient for pane's metadata). Neither has a practical per-inode total limit."

---

## Sources

### Linux Kernel Source (v6.12)
- `include/uapi/linux/limits.h` — VFS xattr size limits (XATTR_NAME_MAX, XATTR_SIZE_MAX, XATTR_LIST_MAX)
- `include/linux/fsnotify.h` — `fsnotify_xattr()` definition, confirms FS_ATTRIB emission
- `fs/xattr.c` — VFS xattr implementation, `fsnotify_xattr()` calls after setxattr/removexattr
- `fs/btrfs/ctree.h` — BTRFS_MAX_XATTR_SIZE, BTRFS_MAX_ITEM_SIZE, BTRFS_LEAF_DATA_SIZE
- `include/uapi/linux/btrfs_tree.h` — struct btrfs_header (101 bytes), struct btrfs_item (25 bytes), struct btrfs_dir_item (30 bytes)
- `fs/xfs/libxfs/xfs_attr.h` — ATTR_MAX_VALUELEN (64KB)
- `fs/xfs/libxfs/xfs_da_format.h` — XFS attribute shortform/leaf/remote format structures
- `fs/bcachefs/xattr.h`, `fs/bcachefs/xattr.c` — bcachefs xattr btree implementation
- `fs/bcachefs/xattr_format.h` — struct bch_xattr (x_val_len is __le16)
- `fs/f2fs/xattr.h`, `fs/f2fs/xattr.c` — f2fs xattr constants and implementation
- `docs/kernel.org/filesystems/ext4/attributes.html` — ext4 xattr block structure
- `docs/kernel.org/filesystems/ext4/eainode.html` — ext4 ea_inode feature
- `docs/kernel.org/filesystems/f2fs.html` — f2fs mount options (inline_xattr, inline_xattr_size)

### Man Pages
- `xattr(7)` — Linux extended attribute overview, namespace classes, VFS limits
- `fanotify(7)` — fanotify event types, FAN_ATTRIB description
- `fanotify_mark(2)` — FAN_MARK_FILESYSTEM, FAN_REPORT_FID requirements

### Filesystem Documentation
- [btrfs documentation](https://btrfs.readthedocs.io/) — nodesize, mkfs options
- [XFS online fsck design](https://docs.kernel.org/filesystems/xfs/xfs-online-fsck-design.html) — attribute fork architecture (shortform/leaf/dabtree/remote)
- [OpenZFS property reference](https://openzfs.github.io/openzfs-docs/man/master/7/zfsprops.7.html) — xattr=sa vs xattr=dir, dnodesize property

### Background
- Giampaolo, Dominic. _Practical File System Design with the Be File System._ Morgan Kaufmann, 1998 — BFS attribute indexing, B+ tree implementation, fs_create_index(), BQuery
