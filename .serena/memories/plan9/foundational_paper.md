# "Plan 9 from Bell Labs" — Pike, Presotto, Dorward, Flandrena, Thompson, Trickey, Winterbottom

Computing Systems, Vol 8 #3, Summer 1995, pp. 221–254.
Local copy: `~/gist/plan9.pdf`

The foundational paper describing Plan 9's architecture. Serves two
purposes for pane: (1) primary source for Plan 9 design decisions,
(2) exemplar of technical writing style.

## Source Documents

- **The paper:** `~/gist/plan9.pdf` — architecture overview (1995)
- **First Edition Programmer's Manual:** https://doc.cat-v.org/plan_9/1st_edition/manual.pdf
  — 1993, AT&T Bell Labs. The permuted index is available; it
  contains the NAME-line entries for every man page in the first
  edition. The index entries themselves are a writing style artifact.
- **Man pages on the web:** https://9p.io/sys/man/ — rio(1),
  plumber(4), pipe(3), exportfs(4), thread(2) are key reference
  pages we've read for voice calibration.

---

## Three Design Principles

The paper states Plan 9's design explicitly:

1. Resources are named and accessed like files in a hierarchical
   file system.
2. There is a standard protocol, 9P, for accessing these resources.
3. The disjoint hierarchies provided by different services are joined
   together into a single private hierarchical file name space.

"The unusual properties of Plan 9 stem from the consistent, aggressive
application of these principles."

These three principles are the foundation. Everything else follows
from applying them uniformly.

---

## Key Architectural Insights

### 9P is the core

"9P is really the core of the system; it is fair to say that the
Plan 9 kernel is primarily a 9P multiplexer."

9P centralizes naming, access, protection, and networking. By
reducing everything to file operations, these concerns are solved
once. Compare object-oriented models where each class of object
must solve naming, access, and networking independently.

### Connection server (cs)

Network-independent name resolution. `cs` is a file system mounted
at a known place. Applications write a symbolic address and service
name to it and read back a list of networks and addresses to try,
ordered by bandwidth. `dial` wraps this into a single library call.

**pane parallel:** The service map (`$PANE_SERVICES`,
`/etc/pane/services.toml`) is pane's cs equivalent — it resolves
service names to transport addresses. The precedence chain
($PANE_SERVICE_OVERRIDES > manifest > $PANE_SERVICES > system default)
is analogous to cs's multi-network resolution.

### Text-format data interchange

"To avoid byte order problems, data is communicated between
programs as text whenever practical."

When binary is necessary, data is decomposed into individual fields,
encoded as an ordered byte stream, and reassembled by the recipient.
The kernel presents process state as text in `/proc/*/status`. The
ps command is trivial — it reformats text files.

**pane parallel:** pane-fs files should return text. postcard
serialization handles the binary wire format, but the filesystem
tier (pane-fs) should present human-readable text, matching the
Plan 9 convention.

### Single process class (rfork)

Plan 9 has one class of process, not two (threads + processes).
`rfork` takes a bit vector specifying which resources to share,
copy, or create anew: name space, file descriptor table, memory,
environment, notes. One primitive handles fork, vfork, thread
creation, and namespace isolation.

"An indication that rfork is the right model is the variety of
ways it is used."

### Terminal model

Terminals have no permanent storage. They access the network's
resources. A user can sit at any terminal and see the same system.
The computing environment is personalized by the namespace, not
by the hardware.

"Plan 9 has one of the good properties of old timesharing systems,
where a user could sit in front of any machine and see the same
system."

**pane parallel:** This maps directly to pane's "host as contingent
server" principle. The local machine is a terminal. State lives
on servers. pane's identity and namespace follow the user, not the
hardware.

### cpu reverse-export

The local terminal exports its devices (display, keyboard, mouse)
to the remote CPU server. The CPU server imports them. Programs
on the CPU server use local devices transparently via the namespace.
This is not rlogin — the remote shell has the same namespace as
the local one, including local device files.

"All local device files are visible remotely, so remote applications
have full access to local services such as bitmap graphics,
/dev/cons, and so on."

### Where the file metaphor breaks down

From the Discussion section — Pike's own assessment:

"/proc is only a view of a process, not a representation. To run
processes, the usual fork and exec calls are still necessary."

"The ability to assign meaning to a command like [cp /bin/date
/proc/clone/mem] does not imply the meaning will fall naturally
out of the structure of answering the 9P requests it generates."

Network interfaces don't use file names for machine addresses
because open/create/read/write don't offer a suitable place to
encode call setup for an arbitrary network. The network interface
is file-like but with a more tightly defined structure (clone +
ctl + data + listen).

**pane implication:** pane-fs should present what naturally fits
the file model (attributes, content, events) and not force
operations that are better served by protocol messages (service
negotiation, obligation handles).

---

## What They'd Do Differently

From the Discussion:

- Replace streams with static I/O queues (streams' configurability
  wasn't used; static queues would be simpler and faster)
- Merge the file server kernel with the regular kernel (separate
  implementation caused double maintenance — drivers written
  twice, bugs fixed twice)

---

## Writing Style Exemplar

Three tiers of the Plan 9 voice, from most expansive to most
compressed:

### Tier 1: The paper (architecture exposition)

**Opens with what things do, not what they are:**
"A central file server stores permanent files and presents them
to the network as a file hierarchy exported using 9P."

**States properties as flat facts:**
"There is no ftp command in Plan 9."
"Plan 9 has no notion of 'teletype' in the UNIX sense."
"There is no superuser."

**Concrete examples immediately after abstraction:**
Every mechanism is illustrated with a command, a file path,
or a code snippet. Never describes a concept without grounding
it in something you can type.

**Justifications follow descriptions, stated as consequences:**
"This is a different style of use from the idea of a 'uniform
global name space'." — the distinction is stated after the
mechanism is described, as a clarifying note, not a preamble.

**Self-critical where warranted:**
"Nonetheless, it is possible to push the idea of file-based
computing too far." — honest about limits without hedging.

**Numbers given matter-of-factly:**
"about 100 megabytes of memory buffers, 27 gigabytes of magnetic
disks, and 350 gigabytes of bulk storage" — no commentary on
whether this is large or small.

**Active voice, present tense, third person for the system:**
"The plumber receives messages..." / "The file server stores..."
First person plural for design decisions: "we adopted" / "we
designed" / "we decided"

### Tier 2: Man pages (reference documentation)

rio(1), plumber(4), pipe(3), exportfs(4) — the compressed
reference variant. Flat declarative sentences, conditionals
stated as facts, no justification or meta-commentary.

"Data written to one channel becomes available for reading
at the other."

"If none has it open, the message is discarded and a write
error is returned to the sender."

### Tier 3: Permuted index NAME entries (maximum compression)

From the First Edition Programmer's Manual. Each entry is a
verb phrase, no articles, describes what the thing does:

    clone − duplicate a fid
    clunk − forget about a fid
    pipe − two-way interprocess communication
    bind, mount, unmount − change name space
    alarm − delay, ask for delayed note
    walk − descend a directory hierarchy
    flush − abort a message
    exportfs − network file server plumbing

**pane application of the three tiers:**
- Tier 1 voice for `docs/architecture.md` (the design spec)
- Tier 2 voice for Rust doc comments (`///`) on public API
- Tier 3 voice for one-line type briefs and `# Brief` sections
