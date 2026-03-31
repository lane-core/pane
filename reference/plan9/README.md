# Plan 9 Reference Material

Selected documentation from Plan 9 from Bell Labs, included as reference
material for pane's distributed architecture and protocol design.

Plan 9 documentation is copyright © 2021 Plan 9 Foundation, released under
the MIT license. See LICENSE in this directory.

## What's here

Man pages and papers that directly informed pane's design:

- **man/** — Selected man pages from the Plan 9 Programmer's Manual
- **papers/** — Selected papers from `/sys/doc/` in the Plan 9 distribution

## How to use

These documents are reference material, not specifications. When implementing
pane features with Plan 9 lineage, read the relevant man pages to understand
the original design intent, then check `pane/plan9_divergences` in serena
memory for how pane adapts the concept.

## Key files for pane development

    man/intro(5)        — 9P protocol (session types, clunk-on-abandon)
    man/namespace(4)    — per-process namespaces (pane-fs model)
    man/import(4)       — remote server mounting (App::connect_remote)
    man/exportfs(4)     — serving a namespace (pane-headless)
    man/alarm(2)        — per-process timers (looper timer model)
    man/factotum(4)     — auth agent (identity model)
    man/plumb(7)        — inter-application messaging (future)

## What's NOT here

- The full Plan 9 source tree (https://github.com/plan9foundation/plan9)
- Lucida fonts (excluded from the MIT relicense)
- Papers published by USENIX/ACM (link to publisher versions instead)

## Source

Plan 9 from Bell Labs, Fourth Edition. Originally developed at Bell Labs
by Rob Pike, Ken Thompson, Dave Presotto, Phil Winterbottom, and others.
Copyright transferred to the Plan 9 Foundation in March 2021.

## Attribution

Bell Laboratories designed the original system. The Plan 9 Foundation
(https://p9f.org/) maintains the archive and holds the copyright. Both
contributions inform pane's distributed architecture and are credited
in pane's API documentation via `# Plan 9` heritage annotations.
