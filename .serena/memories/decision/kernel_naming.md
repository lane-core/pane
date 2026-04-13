---
type: decision
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: normal
keywords: [naming, pane-kernel, translator, host, sys, exokernel]
related: [architecture/kernel, decision/host_as_contingent_server, policy/beapi_naming_policy]
agents: [all]
---

# Naming: pane-kernel (not pane-translator, pane-host, pane-sys)

## Decision

The system interface crate is named **pane-kernel**.

## Rationale

Lane's naming principle: every pane crate name describes what
the crate *does for* the architecture, not what it *relates
to*.

Rejected alternatives:

- **pane-translator** — Lane's initial name. A generalization
  of the Be Translation Kit where the crate translates between
  host system interfaces and pane's portable IR. Rejected
  because Lane refined toward the exokernel concept.
  
- **pane-host** — Roundtable recommendation. Aligns with
  `host_as_contingent_server`. Rejected by Lane: "host" is
  metonymy — it names the other side of the abstraction, not
  what the crate supplies. Every other crate names its own
  functionality (pane-session does sessions, pane-app does
  app framework).

- **pane-sys** — Lane considered this ("system abstraction
  layer"). Rejected: Rust convention `-sys` = raw FFI bindings
  (openssl-sys, libgit2-sys). Would mislead Rust developers.

## Why "kernel"

"Kernel" in the Plan 9 sense: the thin layer providing the
system call interface and device abstraction. Not privileged,
foundational. Plan 9's kernel provides syscalls, #devices,
and namespace machinery; pane-kernel does the same in
userspace as a trait suite.

Lane: "this performs the role that the kernel of plan9's
architecture does, just translated to userspace."

The exokernel framing: pane is an exo-operating system. The
host kernel is the hardware. pane-kernel is the real OS
interface that pane applications interact with. This design
means pane could eventually port as the userspace for a
microkernel OS like Redox.

## Origin

Decision made 2026-04-12 during pane-router/pane-kernel
roundtable. Lane refined through three iterations:
translator → host → kernel, each time sharpening the
concept toward "pane's exokernel."
