# app_server Concurrency Model — Verified from Haiku Source

Key finding for pane server architecture. Verified against Haiku source
2026-04-05.

## Per-Client I/O: Two Separate Ports, No Contention

ServerApp constructor (ServerApp.cpp:125-126):
```cpp
fLink.SetSenderPort(fClientReplyPort);   // write TO client
fLink.SetReceiverPort(fMessagePort);      // read FROM client
```

Two kernel ports per client connection. The ServerApp thread reads from
fMessagePort, dispatches, replies via fLink to fClientReplyPort. All
single-threaded. No lock needed for per-client I/O.

## Cross-App Messaging: Kernel-Mediated, Bypasses app_server

BMessenger::SendMessage writes directly to the target looper's port
(Messenger.cpp:186). app_server never routes inter-app messages.
The kernel port system IS the router.

**Critical difference from pane:** pane's server must route all
inter-pane traffic because connections go through the server's transport.
BeOS got free routing from the kernel. pane needs explicit routing state.

## Desktop Shared State: Reader-Writer Lock (MultiLocker)

Desktop.fWindowLock is a MultiLocker (rwlock):
- LockSingleWindow() = ReadLock — concurrent, used by most operations
- LockAllWindows() = WriteLock — exclusive, used for mutations

Desktop.fApplicationsLock is a separate BLocker for the app list.
Per-ServerApp state (fWindowListLock, fMapLocker) has its own locks.

Read-heavy workloads scaled. Write ops (add/remove window) infrequent.
No lock upgrading supported (documented limitation in MultiLocker.h).

## MessageLooper Pattern: Single-Threaded Dispatch

MessageLooper._MessageLooper() (MessageLooper.cpp:140-165):
```cpp
while (true) {
    receiver.GetNextMessage(code);
    Lock();
    _DispatchMessage(code, receiver);
    Unlock();
}
```

Both Desktop and ServerApp inherit MessageLooper. Each has its own
thread, its own port, single-threaded dispatch. The Lock/Unlock is
for external callers inspecting state, not for internal contention.

## Implication for pane server

pane's server should be an actor (single event loop owning routing
state). Reader threads post to the server's channel; server dispatches
sequentially. WriteHandle mutex stays (multiple writers to same
connection). Routing state mutex disappears — owned exclusively by
the server loop.

This is the MessageLooper pattern translated to calloop.
