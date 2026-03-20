# Plan 9 Research — Tasks 2.1–2.7

Research for pane spec-tightening. Primary sources: Pike's papers on 8½ and acme, the Plan 9 system paper (Pike, Presotto, Dorward, Flandrena, Thompson, Trickey, Winterbottom), "The Use of Name Spaces in Plan 9" (Pike et al.), the plumber paper (Pike), "The Organization of Networks in Plan 9" (Presotto, Winterbottom), man pages for acme(4), cpu(1), exportfs(4), import(1), plumber(4), intro(5).

Sources:

- 8½ paper: <http://doc.cat-v.org/plan_9/4th_edition/papers/812/>
- Acme paper: <https://plan9.io/sys/doc/acme/acme.html>
- Acme (4) man page: <https://9fans.github.io/plan9port/man/man4/acme.html>
- Plumber paper: <https://9p.io/sys/doc/plumb.html>
- Name spaces paper: <https://9p.io/sys/doc/names.html>
- 9P intro: <https://9p.io/sys/man/5/0intro>
- Networks paper: <https://9p.io/sys/doc/net/net.html>
- Plan 9 overview: <https://www.usenix.org/legacy/publications/compsystems/1995/sum_pike.pdf>
- MIT 6.828 Plan 9 lecture: <https://pdos.csail.mit.edu/6.828/2006/lec/l-plan9.html>
- cpu command in Go (FOSDEM 2022): <https://archive.fosdem.org/2022/schedule/event/plan_9_cpu_cmd/>

---

## 2.1 Rio and Acme: Key Ideas and Differences

### Rio (and its predecessor 8½)

Rio is a window system that is a file server. That sentence is the entire design. Rio multiplexes a set of device files — `/dev/cons`, `/dev/mouse`, `/dev/draw` (or `/dev/bitblt` in 8½) — providing each window's client process with a private copy. The mechanism is per-process namespaces: when rio creates a window, it forks a child, mounts itself onto `/dev` in the child's namespace (with `MBEFORE` so the new files shadow the originals), and runs the client. The client reads and writes `/dev/cons` for text I/O, `/dev/mouse` for pointer input, `/dev/bitblt` for bitmap graphics — and has no idea whether it's talking to the kernel, to rio, or to rio running inside rio.

Key technical details:

- `/dev/cons`: ASCII I/O port. Each window gets a distinct instance. Standard read/write.
- `/dev/mouse`: Returns 10-byte messages (1 byte buttons + 4 bytes x + 4 bytes y, little-endian). Blocking read — blocks until state changes or window gains focus.
- `/dev/bitblt`: Protocol file for bitmap graphics. 23 message types encoding raster operations (blit, character draw, etc.). Clients reference bitmaps by integer IDs; all rendering is server-side. Bitmap 0 = the client's window.
- `/dev/window`: Read-only, current window bitmap contents.
- `/dev/screen`: Read-only, entire screen bitmap.
- `/dev/rcons`: Raw console for character-at-a-time input when needed.

The recursive property: rio exports _exactly the same interface it receives from the kernel_. The kernel's internal server implements the same protocol for its single client (rio). So rio can run inside a rio window without any special arrangement — it mounts itself onto `/dev` in the nested window, shadowing the already-mounted rio instance. This is not a trick; it falls out naturally from the design.

Pike calls 8½ "fundamentally no more than a multiplexer." The window creation sequence is 33 lines of C: create a pipe, fork, mount, exec. The `window` shell script for creating new windows from outside was 16 lines — a mount and a redirect:

```
mount -b $'8½serv' /dev
$* < /dev/cons > /dev/cons >[2] /dev/cons &
```

Implementation: 8½ was ~5,100 lines of C (10 source files). The kernel server it talks to was ~2,295 lines plus ~2,214 for graphics operations. Total system including libraries: ~14,651 lines. Binary booted in under one second with instantaneous window creation. Performance compared to X11: bitblt 4x faster, line drawing equal, Unicode text identical to X's ASCII speed.

The concurrency model: 8½ used communicating coroutines — separate coroutines manage mouse input, keyboard input, and each window's state. When idle, 8½ reads the next file I/O request serially from the communication pipe. Pike notes the compromise: "I prefer to free applications from event-based programming. Unfortunately, though, I see no easy way to achieve this in single-threaded C programs." (He later rewrote it in Alef, a concurrent language, then used the thread library when Alef was retired. Rio is the final form.)

Remote transparency: because the interface is purely file-based, remote machines (even those without graphics hardware) can run graphical programs by opening the same files over the network via 9P. A remote machine opens the pipe to 8½, mounts it, and draws. Pike even implemented X11 as an 8½ client for remote machines, with only ~10% performance overhead.

**What rio is NOT:** Rio is not an application framework. It is a multiplexer. It provides the illusion that each program is the only client of the display hardware. Programs don't know they're in a window. Pike: "Window systems are not inherently complicated." The compactness came from: overall simplicity, concise programming interface, fixed user interface, and most importantly the file-based architecture. He concludes: "if the window system is cleanly designed a toolkit should be unnecessary for simple tasks."

### Acme

Acme is something different. Where rio is a transparent multiplexer, acme is an _opinionated integration_ — it merges window system, shell, and editor into one environment. Pike: "Acme is in part a file server that exports device-like files that may be manipulated to access and control the contents of its windows."

The motivation: Pike saw traditional Unix typescript interfaces as "a limited and unimaginative use of the powers of bitmap displays and mice." EMACS offered integration "but with a user interface built around the ideas of cursor-addressed terminals that date from the 1970's." Oberon at ETH Zurich showed "lessons can be applied to a small, coherent system" but "errs too far towards simplicity: a single-process system with weak networking." Acme is the synthesis: Oberon's text-as-interface idea, applied to a networked, multi-process environment, stripped of everything inessential.

Acme's fundamental design principle: "let the machine do the work." And: "a user interface should not only provide the necessary functions, it should also feel right...when one notices a user interface, one is distracted from the job at hand."

**Three-button mouse model:**

- B1 (left): select text
- B2 (middle): execute. The swept text is run as a command. Any text anywhere in acme is a potential command.
- B3 (right): look/plumb. If the text names a file, open it. If it contains an address (`file.c:42`), open at that line. If nothing matches, search for the literal text in the current window.

Single-click expansion: a null selection (click without drag) auto-expands — to a word for B2 (command), to a filename/address for B3 (navigation). This eliminates tedious sweeping. For B3, acme finds the largest string of likely filename characters surrounding the click, checks if the result is an existing file (prepending the window's directory context), and if so opens it. Special case: angle brackets enable include-file resolution — clicking on `<stdio.h>` opens the system include directly.

Sequential search: if you B3-click on an occurrence of a string, acme selects it and scrolls to it. Click again without moving the mouse and it finds the next occurrence. "It isn't even necessary to move the mouse between clicks."

**Tag lines:** Every window has a one-line tag above the body. The tag serves three roles simultaneously:

1. Window identification (directory path or filename)
2. Command palette (Cut, Paste, Put, New, etc. — visible as editable text)
3. Scratch area (right of a vertical bar separator — for ad hoc commands and searches)

There are no menus. There are no toolbars. The tag IS the menu, and it's editable text that you can B2-click. If you want a new command in your "menu," type it in the tag. Pike: "any text anywhere in Acme may be a command."

Pike contrasts this with Macintosh-style menus: "An indication that this style has trouble is that applications provide keyboard sequences to invoke menu selections and users often prefer them." The problem with menus isn't that they exist but that they're fixed, opaque, and detached from the content they operate on. Text commands in tags are editable, visible, contextual, and composable.

**Directory-based context:** Acme has no global "current directory." Every name resolution uses the directory from the window's tag. The string `mammals` in a window tagged `/lib/` resolves to `/lib/mammals`. Commands executed via B2 run in the window's tagged directory. This contextual binding is central to acme's interaction model.

**Command execution:** Non-built-in B2-clicked text executes as a program in the window's tagged directory. stdin = `/dev/null`. stdout and stderr → a window named `dir/+Errors` (created automatically if needed). This makes compiler integration natural: `mk` in a source file's tag produces errors in `+Errors`; right-clicking an error like `parse.c:42: undefined` opens the file at line 42.

**Automatic layout:** Windows tile in columns. Acme places new windows using heuristics: consume blank space, preserve visible text, divide large windows before small ones, position near the action's source. Error windows appear rightward to preserve editing context on the left. The cursor auto-moves to new windows and returns to the previous position when a new window is deleted.

Pike notes placement was iteratively refined: "with each rewrite the system became noticeably more comfortable." This matters — the heuristics aren't derivable from first principles; they emerge from watching people work.

**Filesystem interface:** See section 2.6.

**Implementation:** ~8,000 lines in Alef (concurrent language). Decomposed into communicating processes in a single address space: display/UI, mouse/keyboard forwarders, file server. Each I/O request spawns a dedicated process — "a new dedicated process for each I/O request" — using Alef's synchronous channel communication. Pike: "the code worked the first time" because "the hard work of synchronization is done by the Alef run time system."

**What acme offloads:** Pike emphasizes that acme's UI consistency comes from centralization: "Acme offloads much of the computation from its applications, which helps keep them small and consistent in their interface. Acme can afford to dedicate considerable effort to making that interface as good as possible; the result will benefit the entire system."

People's reaction: "People who try Acme find it hard to go back to their previous environment. Acme automates so much that to return to a traditional interface is to draw attention to the extra work it requires."

### How they differ

| Aspect                 | Rio                            | Acme                                                      |
| ---------------------- | ------------------------------ | --------------------------------------------------------- |
| Philosophy             | Transparent multiplexer        | Opinionated integration                                   |
| Programs know about it | No — they see `/dev` files     | Yes — they use the acme filesystem or are designed for it |
| Window management      | User-driven (mouse gestures)   | Automatic tiling with heuristics                          |
| Text interaction       | Standard terminal I/O          | Text-as-action (B2=execute, B3=route)                     |
| Extensibility          | None needed — it's transparent | Via filesystem interface + external programs              |
| Recursion              | Natural (runs inside itself)   | Not applicable (it's the terminal environment)            |
| Scope                  | Multiplexes existing programs  | Replaces shell + editor + window manager for a workflow   |

Rio is infrastructure. Acme is an environment built on the same philosophical foundations (everything is a file, text is primary) but with strong opinions about how programmers should work.

The deepest difference: rio achieves power through _absence_ of opinion — it adds nothing, so anything composes with it. Acme achieves power through _consistency_ of opinion — everything works the same way, so the user internalizes one interaction model and applies it everywhere. These are not contradictory strategies; they operate at different layers.

---

## 2.2 Per-Process Namespaces

### Mechanism

Every Plan 9 process has its own namespace — its own mapping from path names to resources. The system paper states: "every resource in the system, either local or remote, is represented by a hierarchical file system; and a user or process assembles a private view of the system by constructing a file name space that connects these resources."

Two system calls compose namespaces:

**`bind(new, old, flags)`** — takes a portion of the existing namespace visible at `new` and makes it also visible at `old`. The `new` and `old` are both paths in the same namespace.

**`mount(fd, old, flags)`** — takes a 9P file server on the other end of `fd` and attaches its file tree at `old`. The server can be local (a user-space process serving synthetic files), remote (over the network), or the kernel itself.

Both take a flags parameter controlling composition mode:

- **MREPL (replace)**: the new tree replaces whatever was at `old`
- **MBEFORE**: the new tree appears _before_ existing contents (searched first)
- **MAFTER**: the new tree appears _after_ existing contents (searched last)

MBEFORE and MAFTER create **union directories** — a directory that contains files from multiple sources, searched in the specified order.

**`rfork`** controls namespace inheritance when creating processes. It takes a bit vector specifying which parent attributes are shared versus copied. The namespace is one controllable attribute: shared changes propagate bidirectionally; copied changes remain independent.

An important constraint acknowledged by the designers: "for a process to function sensibly the local name spaces must adhere to global conventions" even though "there is no global name space." Convention replaces enforcement — processes agree on what `/dev` means, what `/bin` means, where `/net` is, but no kernel mechanism forces this.

### What this enables

**Architecture-transparent binaries:** Binaries live in architecture-specific directories (`/mips/bin`, `/386/bin`). At boot, the right directory is bound to `/bin`:

```
bind /mips/bin /bin
```

Shell scripts and programs reference `/bin` — they don't know or care what CPU they're running on. No PATH variable, no per-architecture configuration. The namespace IS the configuration.

**Device virtualization (rio):** As described in 2.1. Rio mounts itself onto `/dev` in each window's namespace. The client sees `/dev/cons` and doesn't know it's talking to rio rather than the kernel. This works because namespaces are per-process — each window can have a different `/dev` without affecting others.

**Recursive composition:** Rio running inside rio is just another mount. The inner rio mounts _itself_ onto `/dev`, shadowing the outer rio's mount, which was itself shadowing the kernel's files. The stack composes naturally. No special recursion support — it's a consequence of the design.

**Devices as files, files as text:** The process device (`/proc`) provides one directory per process with files for memory (`mem`), executable symbols (`text`), control (`ctl` — accepts textual commands like "stop", "kill"), status (`status` — process metadata in fixed text format), and signals (`note`). `cat /proc/*/status` is a crude `ps`. The control files accept ASCII commands, eliminating byte-order issues and enabling remote access.

The console device synthesizes `/dev/cons`, `/dev/time`, `/dev/pid`, `/dev/user` — all on demand, all text. The network device presents protocols as directories (see 2.4). The bitmap device presents mouse state, screen contents, and graphics operations as files.

**Network transparency via import:** `import lab.pc /proc /n/labproc` makes a remote machine's process list appear locally. From there, the local debugger can attach to remote processes — and handle cross-architecture debugging (big-endian MIPS from little-endian i386) because it infers CPU type from executable headers, not assumptions about the local machine.

**Private views of the system:** A testing environment might bind mock files over real ones. A sandboxed process might see a restricted namespace. A remote session might merge local devices with remote storage. The namespace IS the capability set.

**Transparent monitoring:** The `iostats` command interposes on a process's 9P requests within its namespace, reporting usage statistics. No kernel support needed — it's just a mount that relays requests and logs them.

### What you can't do with a global filesystem

With a global `/dev`, every program sees the same devices. You can't give different programs different views of the console without kernel modifications. You can't run a window system as a user-space file server that provides per-window `/dev/cons` instances. You can't transparently overlay remote files on local paths for one process without affecting others. You can't compose system views by stacking mounts.

The names paper makes a claim about what resists the file abstraction, which is worth noting. Process creation (rfork/exec) stays as system calls because "details of constructing the environment of the new process — its open files, name space, memory image, etc. — are too intricate to be described easily in a simple I/O operation." Shared memory requires system calls because file representation would "incorrectly imply remote importability." These are honest admissions of where the abstraction leaks.

Plan 9's per-process namespaces turn the filesystem into a composable abstraction layer rather than a fixed mapping. The namespace IS the configuration, the capability set, and the environment — all manipulable at runtime, per-process, without privilege.

---

## 2.3 The Plumber

### Architecture

The plumber is a user-space file server (~2,000 lines of C) that routes inter-application messages based on pattern-matching rules. It serves files at `/mnt/plumb/`:

- `/mnt/plumb/send` — write a message here to route it
- `/mnt/plumb/rules` — read/write the current rule set
- `/mnt/plumb/edit`, `/mnt/plumb/image`, `/mnt/plumb/web`, etc. — named ports; applications read from these to receive routed messages

Pike: "The plumber takes messages from the send file and interprets their contents using rules defined by a special-purpose pattern-action language."

### Message format

Fixed-format textual header (six lines) followed by free-format data:

```
src            # source application name
dst            # destination port name (may be empty — let rules decide)
wdir           # working directory of the source
type           # data type (MIME-like: "text", "image/gif")
attr           # blank-separated name=value pairs
ndata          # byte count of data section
<data>         # the actual data (ndata bytes)
```

Example:

```
acme
edit
/usr/rob/src
text
addr=27
5
mem.c
```

Library representation:

```c
typedef struct Plumbmsg {
    char *src;
    char *dst;
    char *wdir;
    char *type;
    Plumbattr *attr;
    int ndata;
    char *data;
} Plumbmsg;
```

### Rule language

Rules are grouped into rule sets separated by blank lines. Each rule has three parts: **object**, **verb**, **argument**. All patterns in a rule set must match for the actions to fire. First matching rule set wins.

**Objects** (what to match against): `src`, `dst`, `wdir`, `type`, `data`, `attr`, `arg`

**Verbs:**

| Verb      | Meaning                                             |
| --------- | --------------------------------------------------- |
| `is`      | exact string equality                               |
| `matches` | regex match (sets `$0`, `$1`, `$2`... for captures) |
| `isfile`  | verify the argument names an existing file          |
| `isdir`   | verify the argument names an existing directory     |

**Actions:**

- `plumb to <port>` — send to named port; delivered to all readers (fan-out)
- `plumb client <command>` — start application if port has no reader; message queued until port opens
- `plumb start <command>` — execute command; message discarded
- `data set <value>` — rewrite message data
- `attr add name=value` — add/modify attribute

### Practical examples

**C source file with line number:**

```
type is text
data matches '([a-zA-Z0-9_\-./]+\.c):([0-9]+)'
arg isfile $1
data set $1
attr add addr=$2
plumb to edit
```

This transforms `parse.c:42` → open `parse.c` at line 42 in the editor.

**Image files:**

```
type is text
data matches '[a-zA-Z0-9_\-./]+'
data matches '([a-zA-Z0-9_\-./]+)\.(jpe?g|gif|bit|tiff|ppm)'
arg isfile $0
plumb to image
plumb client page -wi
```

Note the double `matches`: both must match the same text region. This prevents false positives — trailing punctuation like "file.gif." in a sentence is excluded when the second pattern requires a specific extension.

**Header file resolution:**

```
type is text
data matches '([a-zA-Z0-9]+\.h)(:([0-9]+))?'
arg isfile /sys/include/$1
data set /sys/include/$1
attr add addr=$3
plumb to edit
```

Transforms `stdio.h` → `/sys/include/stdio.h`.

**Process ID → debugger:**

```
type is text
data matches '[0-9][0-9]+'
arg isdir /proc/$0
plumb start window acid $0
```

Validates that the number is actually a running process by checking `/proc`.

**Man page references:**

```
type is text
data matches '([a-zA-Z0-9_\-./]+)\(([0-9])\)'
plumb start man $2 $1 | plumb -i -d edit -a action=showdata -a filename=/man/$1($2)
```

Transforms `plumber(1)` into a formatted man page display piped back into the edit port.

### The click attribute

When a user B3-clicks in acme, the message includes a `click` attribute indicating the cursor offset. The plumber uses this to find the "longest leftmost match touching the click position" — extracting the relevant text from the surrounding context. For example, clicking in the middle of `nightmare>horse.gif` extracts `horse.gif` as the filename. The plumber then removes the click attribute and replaces the data with the matched substring before applying further rules.

### What made it powerful

1. **Central authority, user-configurable:** One place defines how content routes. Rules are a text file that users edit. No per-application configuration, no registration APIs, no capability negotiation. Pike: "The plumber, by removing such decisions to a central authority, guarantees that all applications behave the same and simultaneously frees them all from figuring out what's important."

2. **File server design:** Messages pass through regular I/O operations on files. No IPC mechanism to learn. Works over the network identically (the plumber is just another 9P server). Permission model inherits from filesystem ACLs.

3. **Content extraction built-in:** The plumber doesn't just match — it rewrites. `data set`, `attr add`, `isfile` validation — the plumber normalizes paths, extracts line numbers, validates existence, all before routing. Applications receive clean, resolved messages.

4. **Graceful degradation:** If plumbing fails (no matching rule, no open port), the write to `/mnt/plumb/send` returns an error. Applications can fall back to their own behavior (acme falls back to search).

5. **Dynamic:** Rules can be read, appended, replaced, or cleared at runtime by writing to `/mnt/plumb/rules`. Syntax errors are reported as write failures. Changes take immediate effect. No restarts, no registry, no compilation.

6. **Trivial integration cost:** An application that wants to receive plumbed messages just opens and reads from its port file. An application that wants to send messages just writes to `/mnt/plumb/send`. A few dozen lines of code in any language. Acme originally had hard-coded content-recognition rules; it eventually delegated all such logic to the plumber, _shrinking_ its own code.

7. **Not drag-and-drop, not embedding:** Unlike drag-and-drop or cut-and-paste, plumbing requires no explicit user action beyond clicking. Unlike file-extension associations, rules are dynamic and context-aware. Unlike OLE embedding, messages are lightweight headers with data, not embedded application objects.

---

## 2.4 9P — One Protocol for Everything

### The protocol

9P is a byte-oriented protocol for accessing hierarchical file servers. It defines 13 message pairs (T = client request, R = server response):

| Message           | Purpose                                         |
| ----------------- | ----------------------------------------------- |
| Tversion/Rversion | Negotiate protocol version and max message size |
| Tauth/Rauth       | Authenticate connection                         |
| Tattach/Rattach   | Establish root of file tree                     |
| Twalk/Rwalk       | Navigate directory hierarchy                    |
| Topen/Ropen       | Open a file for I/O                             |
| Tcreate/Rcreate   | Create a new file                               |
| Tread/Rread       | Read bytes from a file                          |
| Twrite/Rwrite     | Write bytes to a file                           |
| Tstat/Rstat       | Get file metadata                               |
| Twstat/Rwstat     | Set file metadata                               |
| Tclunk/Rclunk     | Release a fid (file handle)                     |
| Tremove/Rremove   | Delete a file                                   |
| Tflush/Rflush     | Cancel a pending request                        |

Message format: 4-byte size, 1-byte type, 2-byte tag (for multiplexing concurrent requests), then type-specific fields. Integers are little-endian. Text is UTF-8, no NUL termination. Variable-length data uses 2-byte count prefix. Message size is negotiated at connection start via Tversion/Rversion.

**Fids:** 32-bit unsigned integers chosen by the client to identify a "current file" on the server. Analogous to file descriptors but extend beyond open files — a fid can reference any walked-to path, not just an opened file. Multiple concurrent clients on the same connection must coordinate fids.

**QIDs:** 13-byte server-assigned identifiers: 1-byte type (directory, file, append-only, exclusive-use, etc.), 4-byte version (incremented on modification), 8-byte path (unique per file). QIDs let clients detect whether two paths refer to the same file and whether a cached file is stale. "Two files on the same server hierarchy are the same if and only if their qids are the same."

The protocol supports concurrent requests: "A client can send multiple T-messages without waiting for the corresponding R-messages, but all outstanding T-messages must specify different tags."

### What "one protocol" means practically

In Plan 9, every resource is a file server — not metaphorically, but literally serving 9P:

- **Devices**: The kernel serves hardware as files via 9P. `/dev/cons` is a 9P file. So is `/dev/mouse`. So is every device.
- **Window system**: Rio is a 9P server (see 2.1).
- **Plumber**: The plumber is a 9P server (see 2.3).
- **Acme**: Acme is a 9P server (see 2.6).
- **Network stack**: Network connections are files in `/net` — a 9P server. Opening a TCP connection is: open `/net/tcp/clone` to get a connection number, write the destination to the `ctl` file, read/write the `data` file. "All protocol devices look identical so user programs contain no network-specific code."
- **Process control**: `/proc` is a 9P server. Each process has a directory with files for memory, control, status, notes (signals).
- **DNS**: `/net/dns` — a 9P file you write queries to and read responses from.
- **Authentication**: `/mnt/factotum` — a 9P server that handles crypto without exposing keys to applications.

**What this eliminates:**

1. **No ioctl:** Device control is done by writing ASCII commands to control files. No opaque ioctl numbers, no per-device binary interfaces.

2. **No IPC zoo:** No shared memory segments, no message queues, no semaphore sets, no signals for communication, no D-Bus, no COM, no CORBA. One protocol. Write to a file, read from a file.

3. **No protocol per service:** Unix has different protocols for X11, CUPS, PulseAudio, D-Bus, etc. Plan 9 has 9P for all of them. A DNS query, a window resize, a print job, and a CPU export all use the same 13 message types.

4. **Automatic network transparency:** Any 9P server can be accessed over the network by mounting it. No special networking code per service. The plumber works identically whether it's local or on a remote machine.

5. **Universal tools:** `cat`, `echo`, `ls` work on everything. `cat /proc/1/status` shows process info. `echo halt > /dev/reboot` halts the machine. `cat /mnt/plumb/edit` reads the next plumbed message. Shell scripts can drive any service.

6. **Uniform permission model:** File permissions protect everything. No separate ACL systems for different services.

7. **Composition via mount:** Any 9P server can be mounted anywhere in the namespace. This means services compose spatially — you can overlay, union, and rearrange them.

### The cost

9P is a _file_ protocol. Everything must be projected into the file abstraction: reads, writes, directories, metadata. Operations that don't map naturally to this (streaming events, complex queries, transactions) require convention atop the protocol. The plumber's fixed-format text messages, acme's event file encoding, the network stack's clone/ctl/data convention — these are all patterns built on top of 9P's primitives.

The protocol is inherently request/response with no server-initiated push. Clients must poll or block-read for events. (This is why acme's event file blocks until there's something to report.)

The commitment to text-based control (write ASCII commands to `ctl` files) trades parsing complexity for universality. It works because the commands are simple. If the control interface were complex (rich queries, structured responses), the file metaphor would strain.

And there is a tacit cost in _convention_: the clone/ctl/data pattern for network connections, the numbered-directory pattern for processes and windows, the event-file-blocks-until-ready pattern for asynchronous notification — none of these are part of 9P itself. They are design patterns that programs must know. The protocol provides a universal transport; the conventions provide the structure. This is powerful but it means the "one protocol" claim has a layer of convention sitting on top of it that is not formally specified.

---

## 2.5 Distributed Network Architecture

### The network as filesystem

Plan 9 presents networks as filesystems under `/net`. Each protocol (TCP, UDP, IL, Datakit) appears as a subdirectory. The connection establishment sequence:

1. Open `/net/tcp/clone` — the system allocates an unused connection directory and returns its number
2. Write a destination address to `/net/tcp/<n>/ctl` — e.g., `connect 135.104.9.31!512`
3. Open `/net/tcp/<n>/data` for reading and writing

Each connection directory also has `listen` (for accepting incoming calls), `local` and `remote` (connection endpoints), and `status`. All control is via ASCII strings written to `ctl`, eliminating byte-order issues and enabling remote device management.

The Connection Server (CS) at `/net/cs` decouples applications from topology. Write a symbolic name like `net!helix!9fs` and CS returns all viable connection paths:

```
/net/il/clone 135.104.9.31!17008
/net/dk/clone nj/astro/helix!9fs
```

The special network name `net` means "any common network between us and them." Higher-level functions like `dial()` consult CS and try paths sequentially. The authors note that "representing a device as a set of files using ASCII strings for communication" means "any mechanism supporting remote access to files" becomes a network gateway.

### The pieces for distribution

**exportfs(4):** A user-level file server that exports an arbitrary portion of a namespace over 9P. It's a _relay_ — translates 9P requests from a remote client into local file operations and returns the results. Key options:

- `-r root`: serve a subtree
- `-s`: serve the entire namespace
- `-R`: read-only export
- `-P patternfile`: restrict via regex patterns
- `-a`: authenticate before serving

exportfs can serve _any_ namespace, not just on-disk files. This is the key to distribution — it makes any process's view of the world available remotely.

**import(1):** Mount a remote machine's namespace (or a portion of it) into the local namespace. Internally, it connects to an exportfs instance on the remote machine and mounts the result locally.

```
import lab.pc /proc /n/labproc
```

Now `/n/labproc` contains the remote machine's process list. `cat /n/labproc/42/status` shows remote process 42's status.

**cpu(1):** The most important composition of these pieces. The flow:

1. User types `cpu` on their terminal
2. The `cpu` client authenticates to the remote CPU server (factotum handles crypto)
3. `cpu` starts `exportfs` locally, serving the terminal's namespace (including `/dev/cons`, `/dev/mouse`, `/dev/draw` — the window)
4. The remote CPU server starts `rc` (the shell)
5. The remote shell's namespace mounts the exported terminal namespace at `/mnt/term`
6. Standard bindings: `bind /mnt/term/dev/cons /dev/cons`, etc.
7. Architecture-specific `/bin` is rebound for the remote CPU
8. The remote shell now reads/writes the local terminal's window through standard file operations over 9P

Result: the user types in their local rio window. Keystrokes are read from `/dev/cons` by the remote shell over 9P. The remote shell runs programs using the remote CPU. Program output writes to `/dev/cons` over 9P, appearing in the local window. Local files are accessible at `/mnt/term/...`. Remote files are at their native paths.

This is not SSH. SSH gives you a remote shell with remote devices. `cpu` gives you a remote CPU with _your_ devices — your screen, your keyboard, your mouse, your files. The namespace paper describes it: the terminal becomes a file server for the CPU. Traffic is encrypted by default.

### Grid computing

The architecture scales without new mechanisms. Multiple CPU servers sharing a common file server, with each importing `/srv` and `/proc` from all others, "behaves very much like a single large machine." You can:

- Run a process on any CPU server
- See all processes across all machines via imported `/proc`
- Access any machine's services via imported `/srv`
- Use local devices (screen, keyboard) from any machine

This is not a special "grid framework" — it's the natural consequence of per-process namespaces + 9P + mount. The same mechanisms that make rio work (namespace manipulation + file serving) make distributed computing work.

### What this implies for a desktop on Linux

Plan 9's network transparency derives from two properties that Linux does not have:

1. Everything is a 9P file server (one protocol everywhere)
2. Namespaces are per-process and composable without privilege (mount anywhere)

Linux has mount namespaces, but they're heavyweight (require privilege or user namespaces, significant kernel overhead). Linux services speak dozens of protocols. A Linux desktop environment cannot replicate Plan 9's seamless `cpu` — you can't export your Wayland compositor's window over a file protocol and bind it into a remote process's `/dev`.

But the _idea_ of cpu is separable from its implementation. The idea is: computation moves to where the CPU is, while I/O stays where the user is, and the mechanism that makes this work is the same mechanism that makes everything else work (namespace + mount). For a Linux desktop, the question is: what is the equivalent of "the mechanism that makes everything else work"? If pane's internal communication infrastructure is sufficiently uniform, then exposing a remote pane's state locally (or a local pane's state remotely) becomes an instance of the same interop pattern used for everything else, not a special network feature.

---

## 2.6 Acme's Filesystem Interface

### Structure

Acme serves a synthetic filesystem (via 9P) mounted at `/mnt/acme/`:

```
/mnt/acme/
  cons            # stdout/stderr for commands; writes to +Errors window
  consctl         # compatibility stub
  index           # one line per window: id, counts, flags, tag text
  label           # compatibility stub
  log             # stream of window operations
  new/            # special directory — accessing any file here creates a new window
    ctl
    body
    tag
    ...
  1/              # window 1
    addr
    body
    ctl
    data
    event
    errors
    tag
    xdata
  2/              # window 2
    ...
```

### Root-level files

**cons:** Standard and diagnostic output for all commands run under acme. Text appears in a window labeled `dir/+Errors`, created on first write. This replaces the terminal's stdout/stderr — every command's output is captured and displayed, but in acme's own windowed format.

**index:** One line per window. Five decimal numbers (11 chars each, blank-separated): window ID, tag char count, body char count, directory flag (1/0), modified flag (1/0). Then the tag text. Analogous to `ps` output for processes — a snapshot of all window state, parseable by scripts.

**log:** Reports window lifecycle operations since the log file was opened: `new`, `zerox` (clone), `get` (load), `put` (save), `del` (close). Three space-separated fields: window ID, operation, window name. Reading blocks until there's an operation to report. This enables external programs to track acme's window state in real time — a reactive stream via a blocking read.

**new/:** Opening any file in this directory creates a fresh window and returns the corresponding file from the new window's numbered directory. `echo hello > /mnt/acme/new/body` creates a window containing "hello". Opening `new/ctl` and reading it returns the new window's ID, which you can then use to access other files in the numbered directory. This pattern — "open a file in a magic directory to allocate a new resource" — mirrors the network's `clone` file.

### Per-window files

**ctl:** Read returns fixed-format metrics: window ID, tag char count, body char count, directory flag, modified flag, pixel width, font name, tab width, undo flag, redo flag. Write accepts commands:

- `name /path` — set window name
- `addr=dot` — sync address register to current selection
- `dot=addr` — set selection to address register
- `clean` / `dirty` — set modification state
- `del` / `delete` — close window (delete forces even if dirty)
- `get` / `put` — load/save file
- `show` — make selection visible
- `mark` / `nomark` — control undo grouping
- `dump command` — set session recreation command
- `dumpdir directory` — set working directory for dump
- `font path` — change font
- `limit=addr` — restrict subsequent searches
- `scroll` / `noscroll` — control auto-scrolling

**addr:** Write a textual address: line numbers, character offsets (`#n`), regexes (`/pattern/`), compound ranges (`3,7`). Read returns the character offset range as `#m,#n`. This is the random-access positioning mechanism: write an address to `addr`, then read/write through `data` to access that location. The address syntax is the same one the user would type interactively — Sam's address language.

**data:** Used with `addr` for random access to body content. Text written to `data` replaces the addressed text and sets the address to the null string at the end of the written text. Read returns text from the current address. Must contain only whole characters (no partial runes). The file offset is ignored — positioning is via `addr` only.

**xdata:** Like `data` but reads stop at the end of the address range (instead of continuing to EOF).

**body:** The body text. Can be read at any byte offset. Writes always append (offset ignored).

**tag:** The tag text. Same read/write semantics as body.

**event:** The key to external integration. When opened, user actions in the window are reported as structured messages instead of being handled directly by acme. The format:

```
<origin><action> <begin> <end> <flags> <count> [text]\n
```

Origin characters: E (body/tag text change), F (other file operation), K (keyboard), M (mouse)

Action characters:

- D/d: delete in body/tag
- I/i: insert in body/tag
- X/x: B2-click (execute) in body/tag
- L/l: B3-click (look) in body/tag
- R/r: shifted B3-click in body/tag

Flags for X/x: 1=built-in command, 2=null string with expansion (second message follows with expanded text), 8=chorded argument (two more messages follow)

Flags for L/l: 1=interpretable without file load, 2=second message follows, 4=file/window name with address

Programs can **write events back** to acme to invoke default handling. Pike: "changes to the window are reported after the fact; the program is told about them but is not required to act on them." This is event _monitoring_, not event _handling_ — the inversion of the typical UI toolkit model.

**errors:** Writing to this file appends to the window's `+Errors` window (created if needed).

### How external programs use it

**win (acme's terminal analog):** 560 lines, no graphics code. Cross-connects a shell process's I/O to an acme window by monitoring the `event`, `addr`, and `data` files. Text typed after the output point goes to the shell's stdin. Shell output appears at the output point. B2-clicks execute as shell commands. The implementation is trivial because acme handles all rendering, editing, selection, and scrolling — win only manages the boundary between shell I/O and acme's text model.

**Mail reader:** 1,200 lines. Presents a mailbox as a directory of messages. Each message line is B3-clickable to open the message body. Reply creates a composition window. The only complex code is in SMTP/mail protocol handling — acme provides the entire UI for free. Pike: "The only difficult sections of the 1200 lines of code concern honoring the external protocols for managing the mailbox and connecting to sendmail."

**Guide files and edit tools:** Acme stores tools in `/acme/edit/` — single-letter programs (`e` for emit addresses, `x` for extract by regex, `c` for replace, `p` for print) composed via pipes: `e file | x '/regexp/' | c 'replacement'`. A guide file provides usage examples as B2-clickable text. Users edit commands from the guide and execute them. Acme's directory-based command lookup finds binaries in `/acme/edit/` because the guide file's window is tagged with that directory.

**grep/compiler integration:** `grep -n pattern *.c > /mnt/acme/new/body` puts results in a new window. Each line like `parse.c:42:matched text` is B3-clickable — acme + plumber extract the filename and line number, opening the file at the right position.

### Design insights

1. **The filesystem IS the API.** There is no acme library, no acme SDK. You open files, read them, write them. Any language that can do file I/O can control acme. The barrier to integration is approximately zero.

2. **Event monitoring, not event handling.** External programs don't handle keystrokes — acme handles them and reports what happened. Programs react to completed actions, not raw input. This inverts the typical UI toolkit model and means external programs never need to reimplement acme's interaction logic.

3. **Creating a window is opening a file.** `new/ctl` or `new/body` — the side effect of the open creates the window. The metaphor is consistent: windows are directories, window state is files.

4. **The addr/data mechanism is powerful.** It provides arbitrary random access to buffer contents using the same address syntax as the UI (line numbers, regexes, ranges). External programs can search, edit, and navigate programmatically using familiar notation. This is not just "read the file" — it's a query language for text buffers, expressed as file I/O.

5. **The event file is a typed IPC channel.** It reports structured events with origin, action, position, and content. Programs can filter by type, reflect events back for default handling, and compose behavior without reimplementing acme's interaction model.

6. **Applications shrink dramatically.** The mail reader is 1,200 lines. The terminal emulator is 560 lines. The edit tools are trivial pipes. This isn't because acme does a lot of special-case work for them — it's because the filesystem interface is expressive enough that the applications can be small. The shared investment in acme's UI quality pays dividends across every client.

---

## 2.7 Synthesis: How Do These Ideas Inform Pane's Design?

This section is about what the Plan 9 ideas _mean_, not about mapping features between systems.

### What Plan 9 teaches about protocol uniformity and composability

Plan 9's deepest architectural insight is that **a uniform interface enables composition that no amount of special-casing can achieve.** When the window system, the editor, the plumber, the network, the process table, and the device drivers all speak one protocol, you get emergent capabilities that no designer planned:

- Shell scripts control windows (because windows are files, and shell scripts read and write files)
- Remote sessions transparently use local devices (because devices are files, and files can be served over the network)
- Editors integrate compilers through filename conventions (because the plumber reads filenames from editor output through the same file mechanism everything else uses)
- Debuggers attach to processes on foreign architectures by opening directories (because process state is a file tree, and file trees can be imported across machines)

None of these capabilities were individually designed. They _emerged_ from the uniform interface. This is the key: Plan 9's power is not in any single feature but in the fact that all features speak the same language, so any two features can be composed.

The lesson for any system aspiring to composability is: **the number of distinct interfaces is the enemy.** Every additional protocol, every special API, every bespoke IPC mechanism is a boundary across which composition stops. Plan 9 had one boundary (9P) and made everything live inside it. The result was that composition was the default state of affairs rather than something each pair of components had to negotiate.

This does not mean pane needs to use 9P. It means pane needs to minimize the number of distinct interfaces its components must speak to interoperate. The mechanism that provides this uniformity — whether it's a file protocol, a typed message protocol, or something else — matters less than the discipline of having _one_ mechanism that covers the common case.

### What "everything is a file server" actually achieves

"Everything is a file" is often reduced to "you can `cat` things," which trivializes it. What it actually achieves is **a universal composition algebra.** In Plan 9:

- Any service can be mounted anywhere in any process's namespace
- Any service can be interposed upon (put another file server in front of it)
- Any service can be exported over the network
- Any service can be monitored, filtered, or logged without modification
- Any tool that works on files works on any service

The algebra has three operations: mount (attach), bind (rearrange), and serve (provide). These three operations, combined with per-process namespaces, produce every form of composition the system needs.

The emergent property is **substitutability.** Because the interface is uniform, you can replace any component with something that serves the same file tree, and nothing breaks. Replace the real `/proc` with a filtered version that hides certain processes — nothing upstream knows. Replace `/dev/cons` with a logging proxy — the client still reads and writes text. Replace a local file server with an imported remote one — the programs using it don't change.

This matters for a desktop environment because a desktop is fundamentally a _composition of services_: a compositor, a shell, a file manager, a notification system, a clipboard, an application launcher, a settings manager. If each of these speaks a different protocol (which is the status quo on Linux — Wayland, D-Bus, X11 clipboard, freedesktop protocols, etc.), then composing them requires N×M adapter code. If they can be made to speak a common language, composition becomes the default.

### What the plumber teaches about content-based routing as infrastructure

The plumber's most important property is not its rule language. It's the decision to make content routing a **system service** rather than an **application feature.**

Before the plumber, each application had to decide for itself what to do with text that might be a filename, a URL, an error message, a man page reference. Every application did this differently, partially, and inconsistently. The plumber centralized this intelligence and gave every application the same behavior for free.

The implications:

1. **Routing rules are user-configurable infrastructure**, not application behavior. The user can change how filenames are handled system-wide by editing one text file. No per-application settings, no plugins, no configuration formats to learn.

2. **Applications get simpler.** Acme originally had hard-coded content recognition; it delegated this to the plumber and got _smaller_. When routing is infrastructure, applications can focus on their actual job.

3. **The routing layer transforms content, not just dispatches it.** The plumber rewrites data, adds attributes, validates file existence, extracts substrings — all before delivery. The receiving application gets a clean, resolved message. This is why Pike calls it a plumber, not a router: it does work on the data flowing through it.

4. **The trigger is invisible.** Users click on text. They don't "invoke the plumber." The plumber is infrastructure that makes clicking on a filename _do the right thing_ without the user knowing routing is happening. The best infrastructure is the kind you don't notice.

For any system that wants its components to interoperate through content: the lesson is that routing should be a service, not a library; it should be configurable by the user, not only by developers; and it should transform content, not just forward it.

### What acme teaches about filesystem-as-API for a program's state

Acme's filesystem interface is remarkable not just because it exists, but because of what it does to the _relationship between the program and its extensions._

In most extensible programs (Emacs, VS Code, Vim), extensions live _inside_ the program. They use the program's language, link against its API, run in its process, and must be updated when the program changes. The extension boundary is the language binding.

In acme, extensions live _outside_ the program. They are separate processes that read and write files. They can be written in any language. They don't link against acme. They don't run in acme's process. They need not be updated when acme's internals change (only when the filesystem interface changes). The extension boundary is the filesystem.

This produces concrete effects:

- **win** (terminal emulator) is 560 lines with no graphics code, because acme provides all rendering through the body file.
- **The mail reader** is 1,200 lines, almost all of which is mail protocol handling, because acme provides all UI through the filesystem.
- **grep integration** requires no code at all — just output format conventions (`file:line:text`) that the plumber and B3-click already understand.

The addr/data mechanism deserves particular attention. It's not just "read the file contents." It's a query language embedded in file I/O: write a regex to `addr`, and the next read from `data` returns the matching text. Write replacement text to `data`, and it replaces the addressed region. This turns file I/O into a programmatic editing API with the full power of Sam's address language.

The event file is equally important. It inverts the normal relationship between a program and its event loop: instead of the program _handling_ events (and therefore needing to reimplement all of the host program's behavior), the program _monitors_ events and can reflect them back for default handling. An external program that intercepts B2-clicks on a window can decide to handle some commands itself and let acme handle the rest — by writing the event back to the event file. This is compositional: the external program adds behavior without replacing the defaults.

The lesson: when a program exposes its state as a filesystem, the barrier to extension drops to zero. Any language, any process, any machine (over the network) can participate. The filesystem is the universal FFI.

### Where pane is continuous with Plan 9, where it necessarily departs

**Continuity in ideas:**

Pane's core conviction — that the system should be built from a small, principled core (typed protocols, filesystem interfaces, composable servers) from which the entire experience is derived by first principles — is Plan 9's conviction. The insistence that interfaces expose semantics rather than syntax descends from Plan 9's commitment to text-based, human-readable interfaces that make the system's structure visible to users and scripts alike.

Tag lines on every pane are acme's tags. B2-executes and B3-routes are acme's mouse model, with B3 generalized from "look in this file" to "route through the system." The idea that any visible text is a potential command comes directly from acme. The automatic tiling with heuristics comes from acme's column model.

The idea of a content-based routing service that transforms and dispatches messages based on user-editable rules is the plumber. The idea that this should be a system service, not an application feature, is the plumber's deepest contribution.

The idea that the system's state should be accessible as a filesystem, so that shell scripts and external tools get the same access as native components, is acme's filesystem interface generalized to the whole desktop.

**Necessary departures:**

Pane lives on Linux, not Plan 9. This changes three things fundamentally:

_No per-process namespaces as a composition primitive._ Plan 9 builds everything on bind/mount into per-process namespace. Rio IS this: it mounts itself onto `/dev` in each window's namespace. This is how a window system becomes invisible — each process's namespace gives it private devices. Linux mount namespaces exist but are heavyweight, require privilege, and aren't designed for per-window manipulation. Pane cannot achieve rio's namespace-mediated transparency. Instead, it provides an enriched abstraction: each pane has a typed protocol connection to the compositor. The effect is similar (per-pane isolation, per-pane communication) but the mechanism is socket protocol rather than filesystem mount.

_No single protocol for the ecosystem._ Plan 9 could mandate 9P because it was a complete OS. Pane is a desktop environment on Linux, where the ecosystem speaks Wayland, D-Bus, X11 clipboard, freedesktop protocols, and dozens of other things. Pane cannot replace these; it must bridge them. The designer's own framing captures this: "why not conceive of pane-route as a superset of the network kit, which ought to encapsulate the variety of protocols by which processes (and users) communicate with each other, providing nice abstractions over the gnu/linux base." This is not Plan 9's "replace everything with 9P." It's "provide a uniform internal interface and translate at the boundaries." The protocol bridge model (abstracting D-Bus, abstracting 9P, abstracting whatever else is needed into native pane interfaces) is the pragmatic response to a world where you don't control the whole stack.

_A richer content model._ Plan 9's interfaces are text. Acme's body is text. The plumber's messages are text with metadata. This is powerful and universal but it's also a constraint — structured data must be projected into text, queries are regex-based, layout is implicit. Pane's designer has articulated a richer vision: typed protocols, semantic interfaces that orient users to the meaning of what is happening. This goes beyond Plan 9's text universalism toward something that can represent structure natively while still being composable. The challenge is preserving composability when the interface has more structure than flat text.

**The deepest tension:**

Plan 9 achieves composability through _uniformity of representation_: everything is a file, everything is text, everything speaks 9P. This uniformity means that anything composes with anything else automatically. But it also means that everything must be projected into the file/text/9P model, which is lossy for structured data.

Pane aspires to composability through _typed protocols and semantic interfaces_, which preserve more structure but create a higher bar for interoperation. Two services that speak different typed protocols don't automatically compose the way two 9P file servers do. The typed protocol provides correctness guarantees (you can't send a malformed message), but it reduces the "just works" universality that Plan 9's uniform text interface provides.

The filesystem interface (Plan 9's gift) resolves part of this tension: even if pane's native protocol is typed and structured, the filesystem projection gives shell scripts and external tools the universal text-based access that Plan 9's design proved essential. The two interfaces — typed for native components, filesystem for everything else — are complementary, not redundant. The typed protocol provides safety; the filesystem provides universality. Plan 9 teaches that the universal interface is not optional, even when you have a better native one.
