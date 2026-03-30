# Wisdom from the Be Engineers

Curated insights from the Be Newsletter (1995-1999), organized by theme.
These are the voices of the people who built BeOS, explaining *why* they
made the choices they made.

---

## 1. The Threading Model: Why Per-Window Threads

The single most distinctive architectural decision in BeOS was giving every
window its own thread. This wasn't an accident -- it was a deliberate
response to the limitations the engineers had seen in single-threaded systems.

**Benoit Schillings** (Issue 1-2, "Programming Should Be Fun"):

> "A single window always has two threads associated with it: One on the
> client side, which is used when drawing in the window, and one on the
> application server side, which is used to execute the client's requests.
> This simple idea takes advantage of the dual-CPU architecture without the
> application programmer having to know about it. In a certain sense, for the
> duration of a graphic operation, one of the CPUs turns into a super graphics
> coprocessor."

The key insight: threading was not exposed as a burden on the developer but
as an invisible performance multiplier. The system's threading model made
parallelism *structural* rather than opt-in.

**Peter Potrebic** (Issue 1-4, "Summer Vacations and Semaphores"):

> "In the Be operating system each window has its own client-side thread. In
> addition, every application has a dedicated thread, called the main thread.
> In many other desktop systems, applications typically consist of a single
> thread of execution. On the BeBox, applications are inherently
> multithreaded, simply by virtue of putting up a few windows."

And the two commandments that made this workable:

> "Be Commandment #1: Thou shalt not covet another thread's state or data
> without taking proper precautions."

> "Be Commandment #2: Thou shalt not lock the same objects in differing
> orders."

Potrebic on three strategies for safe cross-thread access:

> "That gives you three different methods to access data safely, and there
> are probably more. The method to use depends on the situation. The important
> thing is to understand and think about these issues."

The three methods: (1) control the data from one thread, (2) use messaging
to request information, (3) lock the window before accessing data. The
messaging approach -- sending a message and letting the recipient respond
when in a consistent state -- is the one that scales best. This is exactly
what session types formalize.

**George Hoffman** (Issue 2-36, "Tips on Writing Efficient Interfaces"):

> "The idea behind the window thread is that there will always be a thread
> ready to react to a message from another window, or user input, or an
> app_server update message."

> "Keeping a window locked or its thread occupied for long periods of time
> (i.e. over half a second or so) is Not Good. If a window thread becomes
> unresponsive, and the user continues to provide input... its message queue
> will fill up. If this happens, the app_server will start to throw away
> update messages and input intended for the window, and this can cause
> erratic behavior."

The lesson: the threading model demands *discipline* about what runs on
the window thread. Heavy work goes in spawned threads. The window thread
must stay responsive. This is the same discipline pane enforces structurally
through session types.

**Pane implication:** The per-window thread model is the right one, but
BeOS enforced it by convention. Pane can enforce it by construction through
session types that make the message protocol explicit and prevent blocking
the component's event loop.

---

## 2. The Client/Server Architecture: How It Actually Works

**Bob Herold** (Issue 1-5, "From Power Up to the Browser"):

> "The shared library together with the various servers implement the API.
> The library uses the kernel's port mechanism and shared memory to
> communicate with the servers. API calls are converted into messages and
> sent through a port to a thread in the appropriate server. Servers usually
> allocate a single thread to manage each client, allowing for concurrent
> access to the functionality each server provides."

This is the fundamental pattern: the client-side library converts API calls
into messages sent via kernel ports. The server allocates a thread per client.
Results come back via another port. The developer sees a synchronous API
but the implementation is asynchronous message passing over ports.

**George Hoffman** (Issue 2-36) on the two kinds of app_server calls:

> "There are two kinds of app_server calls: asynchronous and synchronous. An
> asynchronous call sends off its message to the app_server and returns to
> the caller; in this way the server can process the client's request while
> the client prepares more data to be sent. On a multiprocessor machine, the
> client and server threads will be scheduled in parallel. This is Good."

> "Synchronous calls are calls that require a response... Synchronous calls
> are much slower than asynchronous calls, for several reasons. First, the
> Interface Kit caches asynchronous calls and sends them in large chunks at a
> time. A synchronous call requires that this cache be flushed."

The insight: batching asynchronous calls is essential for performance. The
Interface Kit accumulated drawing commands and flushed them in chunks,
turning many small messages into fewer large ones. Synchronous calls forced
a flush and a round-trip.

**Pane implication:** pane-route should batch messages where possible, and
the protocol between components and servers should be predominantly
asynchronous. Session types can distinguish between fire-and-forget
messages and request-response pairs.

---

## 3. The Benaphore: Optimizing the Common Case

**Benoit Schillings** (Issue 1-26, "Benaphores"):

> "Typically, acquiring and releasing a semaphore takes about 35
> microseconds. This overhead can add significantly to the cost of
> manipulating the data structure. It's not unusual for the semaphore calls
> to take more time than the critical section itself."

> "An uncontested trip through the critical section that's protected by a
> benaphore has an overhead of under 1.5 microseconds. That's twenty times
> faster than using a semaphore."

The benaphore is a combination of an atomic variable and a semaphore. The
atomic variable is checked first; the semaphore is only acquired if there
is actual contention. In the common (uncontested) case, you skip the
kernel entirely.

> "Of course, the benaphore imposes an overhead of its own (the atomic
> variable check); this overhead is unnecessary if there's a lot of
> contention for the critical section. But even in the worst case, the
> atomic variable overhead gets lost in the noise of the context switch."

**Pane implication:** Optimize for the common case. In pane's threading
model, most lock acquisitions will be uncontested (a component's own data
accessed from its own thread). The locking primitives should fast-path
this case.

---

## 4. The Networked BMessenger Vision

**Bradley Taylor** (Issue 1-7, "Everything You Wanted to Know About Be
Networking"):

> "We also would like to develop some Be-specific protocols, so that Be
> applications can be easily extended to the network. A simple way to do
> this is to extend the Be Messenger class, so that it could be instantiated
> targeting either a local or a remote application. Applications on your
> desktop could easily take advantage of services not otherwise available
> locally, such as faster CPU speeds, distributed processing, heavyweight
> database services, or exotic I/O devices."

This was 1996. The vision was that BMessenger -- the same abstraction used
for local inter-thread communication -- would transparently work across
the network. The messaging infrastructure would be the same whether the
target was in-process, in another process, or on another machine.

**Pane implication:** This is exactly pane-route's design goal. The
communication infrastructure should be location-transparent. A component
should not need to know whether its correspondent is local or remote.

---

## 5. Memory Costs: The Hidden Price of Objects

**Pierre Raynaud-Richard** (Issue 4-46, "The Hidden Cost of Things"):

The precise memory costs of BeOS objects, from the engineer who measured
them:

- Empty BMessage: 64-128 bytes
- BHandler: 64-128 bytes
- BView: ~1600 bytes
- Spawning a thread: ~16.25 KB (mostly kernel stack)
- Running a BLooper: ~20.75 KB
- Creating a window (with both threads): ~56 KB
- Showing a window: ~70 KB
- New BApplication team: ~214 KB

> "Remember, these numbers don't include whatever memory will be used by the
> objects once you start using them (and that can be a lot, depending on what
> you do)."

The insight: threads are expensive (~20KB each). Windows are very expensive
(~70KB). This is why the per-window-thread model is a deliberate choice
rather than per-widget threading -- you need to choose the right granularity.

**Pane implication:** Choose the threading granularity carefully. A thread
per pane (window/panel) is the right level, not a thread per widget. The
~20KB per-thread overhead is the tax for concurrency; spend it wisely.

---

## 6. The System Architecture: Modularity That Scales

**Erich Ringewald** (Issue 1-1):

> "We wanted to make available to developers a collection of technologies we
> thought were cool, technologies which weren't showing up on the mainstream
> platforms. We wanted to make the machine lean, cheap, and fast -- things
> not many people are accusing the mainstream platforms of being."

> "There is just no excuse for a multitasking personal computer which is
> expected to maintain user responsiveness, display multimedia data types,
> and manage a sophisticated communications protocol to the net not to have
> more than one processor to throw at the jobs."

**Jean-Louis Gassee** (Issue 2-35, "Scalability or Modularity?"):

> "Architectural features such as the client-server architecture inside the
> BeOS make it relatively easy to reconfigure modules and to remove functions
> not needed when addressing a different target application."

> "Most systems only 'scale' upwards, they bleed to death when a cut is
> attempted in order to downscale them."

The BeOS could be slimmed to fit on a 1.44MB floppy. This wasn't because
they built a "lite" version -- it was because the client-server architecture
meant you could simply leave out servers you didn't need. The modularity
was architectural, not bolted on.

**Pane implication:** pane's server architecture should be composable in
the same way. If you don't need audio, don't start the audio server.
The system should work with any subset of its servers.

---

## 7. The Fresh Start Advantage

**Jean-Louis Gassee** (Issue 1-7, "Strategy"):

> "On the minus side, we forfeit a legacy of thousands of applications...
> On the plus side, a fresh start frees us from the baggage of a decade or
> more of incremental fixes and extensions, from many accumulated layers of
> software silt."

> "If, in a short period of time, our noble and worthy elders could
> replicate the advantages we offer, we'd be in a position known as picking
> up dimes one foot ahead of the steamroller."

Gassee's test for whether a fresh start is justified: can the incumbents
replicate your advantages faster than you can build an ecosystem? If their
legacy prevents them from matching your architecture for years, you have a
window.

**Gassee** (Issue 1-4, "Heterogeneous Processing") on lessons learned from
their DSP mistake:

> "People developing the system now have to contend with two programming
> models and two pieces of system software and the coordination headaches
> between them."

They tried putting DSPs alongside CPUs and abandoned it because the
heterogeneity created too much complexity. The lesson: a uniform
programming model is worth more than raw heterogeneous performance.

**Pane implication:** One communication model (pane-route), one threading
model (per-component BLooper pattern), one extension model (filesystem-based
typed interfaces). Uniformity of abstraction over theoretical optimization.

---

## 8. Developer Experience as Design Philosophy

**Benoit Schillings** (Issue 1-2):

> "Why is the BeBox fun to program? I guess the main reason is that most of
> the operating system design and application framework was done by people
> with experience in writing real programs. As a result, common things are
> easy to implement and the programming model is CLEAR. You don't need to
> know hundreds of details to get simple things working."

**Erich Ringewald** (Issue 1-1):

> "We really don't think any system out there has delivered on that promise
> as we have. (X Windows is not very popular here at Be)."

> "I really think the BeOS is a system with the look and feel elegance of
> the Mac, with a real OS underneath."

**William Adams** (Issue 2-36):

> "Peter Potrebic, Be programmer extraordinaire, has put quite a few nifty
> features into the BeOS in very small packages. BLooper, BMessage, BHandler,
> and BMessageFilter just to name a few."

The small, composable primitives of the messaging system (BLooper, BMessage,
BHandler, BMessageFilter) were designed to be simple individually but
powerful in composition. A BMessageFilter could intercept messages before
they reached handlers, enabling cross-cutting concerns without subclassing.

**Pane implication:** The API surface should be small, the primitives
composable, and common operations trivial. Developer experience is not a
polish pass -- it is a design constraint from day one.

---

## 9. What Changed / What Went Wrong

**Gassee on DSPs** (Issue 1-4): Tried heterogeneous DSP+CPU, abandoned it.
Two programming models and two OS layers was too much complexity for too
little gain. Switched to homogeneous PowerPC.

**Network server as user-space process** (Issue 1-7): TCP/IP was
implemented as a server rather than in-kernel, which meant slight
performance penalty but better isolation. Trade-off: sockets were in a
separate namespace from file descriptors, breaking the UNIX convention.

**App_server synchronous calls** (Issue 2-36): The original design allowed
both sync and async calls to the app_server. Engineers discovered that sync
calls were vastly more expensive due to cache flushing and round-trip
latency. The guidance evolved to: avoid sync calls, cache data client-side,
use async by default.

**BMessage(BMessage*) constructor** (Issue 4-46, Owen Smith): A convenience
constructor that took a pointer instead of a reference. Seemed harmless.
Created subtle bugs through implicit conversions and const-correctness
violations. Deprecated.

> "Both this problem and the memory leak could be solved by declaring the
> constructor to be explicit. ... However, between these issues, the const
> issue, and redundancy, we chose to deprecate this constructor instead."

**BFS and node_ref on disk** (Issue 3-24, Dominic Giampaolo):

> "I really want to STRONGLY discourage anyone from storing [entry_refs and
> node_refs] on disk. ... If you store them on disk and then someone moves
> that file to another volume, the entry/node_ref is no longer valid."

> "The BeOS does not have Mac aliases because they are not supportable in a
> networked environment that uses NFS or CIFS (and we were trying to think
> ahead a little bit)."

**Pane implication:** Think about the networked case from the start. Don't
introduce convenience features that create hidden state dependencies.
When in doubt, deprecate rather than accumulate design debt.

---

## 10. File Types and the Integrated System

**Pavel Cisler** (Issue 3-10, "File Types, the Tracker, and You"):

> "Even though we strongly believe that using file types to control document
> opening/drag-and-drop/application launching is a great way to enhance the
> user experience in BeOS -- and we will be enforcing it more and more in
> future releases -- we don't want the user to be unable to open a document
> just because a file type is wrong."

The file type system (MIME types stored as attributes) was the glue that
held the integrated experience together. Applications declared what types
they handled; the Tracker used this to offer intelligent "Open With" menus.
But they also provided an escape hatch (Control-drag to bypass type
checking).

The principle: type systems should guide behavior, not prevent it. Strict
enough to be useful, flexible enough to not trap users.

---

## 11. The BLooper / BMessage / BHandler Architecture

**William Adams** (Issue 2-36) on the messaging primitives:

> "The well-versed BeOS programmer should know that we think threads are the
> best thing on Earth. Thread programming can often times be messy and
> difficult to keep straight. So the BeOS provides the BLooper class to make
> things simple. The general idea is that a BLooper controls something, and
> the only way to affect changes on that something is to send a message."

On BMessageFilter as a composition mechanism:

> "You didn't have to sub-class the BLooper or BHandler classes. You did
> have to sub-class BMessageFilter, but in a growing system, sub-classing a
> nice small object that is unlikely to change is probably easier than
> sub-classing a highly active object like BWindow or BApplication."

The messaging architecture had four key primitives:
- **BLooper**: owns a thread and a message queue; dispatches messages
- **BHandler**: receives messages within a BLooper's context
- **BMessage**: the data container -- typed, nestable, serializable
- **BMessageFilter**: intercepts messages before dispatch

This quartet is what session types formalize. The BLooper is the component
with a thread; BMessages are the protocol; BHandlers are the endpoints;
BMessageFilters are middleware.

---

## 12. Pavel Cisler on Thread Synchronization Patterns

**Pavel Cisler** (Issue 3-33, "Fun with Threads, Part 2"):

> "Lock() is designed to handle being called on Loopers that have been
> deleted, so if the window is gone, the lock will fail and we'll just bail."

> "Using Lock() on the spawning thread like this is the most common way of
> synchronizing. It serves two purposes: it serves as a lock for the shared
> state, allowing the thread to access state in the spawning thread in a
> mutually exclusive way; and it allows the thread to exit in a reasonably
> clean way after the spawning thread dies itself."

On a subtle aliasing bug:

> "If, while you're snoozing, your spawning window is deleted and a similar
> one is created in its place, aliased to the same pointer value... You may
> use a more sophisticated locking technique, involving a BMessenger -- a
> messenger-based lock has a more elaborate locking check and handles an
> aliasing issue like this completely."

The BMessenger-based lock was stronger than a raw pointer lock because
BMessenger included identity checking, not just pointer validity. This
is the kind of safety that session types provide by construction.

---

## 13. Performance Philosophy

**Benoit Schillings** (Issue 1-2) on threading for performance:

> "In the same way, when you do a lot of small writes to a file, the actual
> writing to the file is performed by another thread. In this case, the
> second CPU becomes a dedicated I/O processor."

**William Adams** (Issue 2-40) on kernel-level operations:

> "In short, we do the work, so you don't have to. We speak to CPU vendors
> and find those special commands that make atomic actions work on 603, 604,
> x86 and other CPUs."

> "Fine grained locking is typically something you do in the kernel space;
> in user space, you're better off with the semaphore and Benaphore
> techniques."

The philosophy: the system does the hard concurrency work. Application
developers get simple, safe primitives (BLooper, benaphore, BMessenger).
The kernel handles the architecture-specific details.

---

## 14. Gassee on What Makes a Platform

**Jean-Louis Gassee** (Issue 4-10, "The First Media Kit Conference"):

> "I've often thought that, in our business, you know you've done something
> right when your platform is perverted. By which I mean... that programmers
> or normal people use your product in ways you hadn't thought of."

> "Basic BeOS features such as symmetric multiprocessing, pervasive
> multithreading, and a 64-bit journaled file system give it rendering
> power. The Media Kit, on the other hand, gives the BeOS expressive power,
> the ability to write new music -- that is, new applications that aren't
> necessarily as easy to write, or perhaps impossible, on other sets of APIs."

The distinction between *rendering power* (raw capability) and *expressive
power* (enabling new kinds of applications) is the distinction between
infrastructure and architecture. BeOS's threading and filesystem were
rendering power. The messaging system and kits were expressive power.

**Pane implication:** pane-route is the expressive power layer. The Wayland
compositor and session types are rendering power. The architecture should
enable applications that are impossible or impractical on conventional
desktops.

---

## Summary: What BeOS Got Right That Pane Should Preserve

1. **Per-component threading** with the system managing concurrency, not
   the developer
2. **Message passing as the universal communication pattern**, with the
   same abstraction working locally and (in vision) remotely
3. **Client/server split** where libraries convert API calls to messages
   and servers allocate threads per client
4. **Small, composable primitives** (BLooper, BMessage, BHandler,
   BMessageFilter) rather than monolithic frameworks
5. **Optimize for the common case** (benaphores, async batching) while
   keeping the API simple
6. **Architectural modularity** that allows the system to scale up (more
   processors) or down (embedded) without API changes
7. **Developer experience as a first-class design constraint**, not an
   afterthought
8. **Type systems that guide rather than prevent** -- MIME types for files,
   message codes for IPC, with escape hatches for edge cases

## What BeOS Got Wrong That Pane Should Avoid

1. **Convention over enforcement** -- threading safety was a commandment,
   not a compile-time guarantee. Session types fix this.
2. **DSP heterogeneity** -- having two programming models is worse than
   having one. Keep the abstraction uniform.
3. **Convenience constructors** that enable implicit conversions -- the
   BMessage(BMessage*) debacle shows why explicit is better.
4. **Storing identity tokens on disk** -- node_refs and entry_refs that
   became invalid when files moved across volumes.
5. **Sync calls in an async architecture** -- the app_server's synchronous
   calls were consistently slower and should have been avoided from the
   start.
