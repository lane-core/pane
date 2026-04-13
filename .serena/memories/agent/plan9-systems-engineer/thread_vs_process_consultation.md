---
type: analysis
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [thread-per-pane, process-per-pane, rfork, namespace, isolation, hybrid, predicate, DeviceRegistry, rio, winshell]
related: [architecture/kernel, architecture/compositor, architecture/fs, decision/host_as_contingent_server, policy/feedback_per_pane_threading, agent/plan9-systems-engineer/linux_namespace_analysis]
agents: [plan9-systems-engineer]
sources: [Plan 9 fork(2), Plan 9 thread(2), rio/wind.c, rio/rio.c, libthread/create.c]
---

# Thread-per-pane vs Process-per-pane: Plan 9 Consultation

## Key findings

1. **Rio used separate processes per window.** `winshell()` in
   `wind.c:1355` calls `rfork(RFNAMEG|RFFDG|RFENVG)` — copy
   namespace, fd table, environment — then mounts the window's
   virtual devices and execs the shell. Each window is a real
   process with a real private namespace.

2. **Plan 9 namespace copy was cheap (~microseconds)** because
   the mount table was a flat linked list of ~10-20 entries.
   Linux `unshare(CLONE_NEWNS)` is 10-50x more expensive
   (50-200us) because it copies the full VFS mount tree.

3. **Plan 9 had threads (libthread) but they shared namespaces.**
   `threadcreate()` makes coroutines within a proc; they share
   everything including the mount table. `proccreate()` makes
   a new proc with shared memory. Namespace isolation required
   process-level separation.

4. **rfork flags were composable but process-granular.** No flag
   combo gives per-thread namespace isolation. RFNAMEG without
   RFPROC modifies the current process's namespace (useful after
   fork, not for thread isolation).

5. **Hybrid model recommendation: threads default, processes for
   isolation.** The predicate model (HashSet filter on
   DeviceRegistry) is functionally equivalent to a mount table
   with fewer entries. The enforcement differs (userspace
   predicate vs kernel mount table walk) but the interface is
   identical (Dev trait). Process isolation via
   unshare(CLONE_NEWNS|CLONE_NEWUSER) available on Linux as
   upgrade path for untrusted panes.

6. **Essential character preserved.** Plan 9's namespace model is
   about "the namespace is the interface and you can customize it
   per-process." pane preserves this: customization is per-pane
   (predicate or real namespace), interface is uniform (Dev trait).
   What would violate Plan 9's spirit is side channels that bypass
   the namespace entirely — as long as the two-tier pattern holds,
   the spirit survives.

## Cited sources

- `reference/plan9/src/sys/src/cmd/rio/wind.c:1355` — rfork in winshell
- `reference/plan9/src/sys/src/cmd/rio/rio.c:290` — rfork in rio main
- `reference/plan9/man/2/fork` — rfork flags
- `reference/plan9/man/2/thread` — thread/proc model
- `reference/plan9/src/sys/src/libthread/create.c` — proccreate/threadcreate impl
