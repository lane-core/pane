# Plan 9 Reference Material

Selected documentation from Plan 9 from Bell Labs, included as reference
material for pane's distributed architecture and protocol design.

Plan 9 documentation is copyright © 2021 Plan 9 Foundation, released under
the MIT license. See LICENSE in this directory.

## What's here

The complete Plan 9 Programmer's Manual (man pages) and paper sources
from the Fourth Edition distribution:

- **man/** — All man page sections (1-8), 565 pages in troff format
- **papers/** — Troff sources (.ms) and HTML for the system papers

Binary-generated files (PostScript, PDF) are excluded to keep the
repo lean. The troff sources are the authoritative versions.

## How to use

These documents are reference material, not specifications. When implementing
pane features with Plan 9 lineage, read the relevant man pages to understand
the original design intent, then check `pane/plan9_divergences` in serena
memory for how pane adapts the concept.

Man pages are in troff format. Read them raw or render with `nroff -man`.

## Key files for pane development

    man/5/0intro        — 9P protocol (session types, clunk-on-abandon)
    man/5/clunk         — clunk message (PaneCreateFuture drop semantics)
    man/4/namespace     — per-process namespaces (pane-fs model)
    man/4/import        — remote server mounting (App::connect_remote)
    man/4/exportfs      — serving a namespace (pane-headless)
    man/2/sleep         — alarm(2): per-process timers (looper timer model)
    man/4/factotum      — auth agent (PeerIdentity model)
    man/6/plumb         — inter-application messaging (future)
    man/4/rio           — window system (compositor comparison)
    man/2/thread        — libthread (per-process event loops)
    man/3/draw          — draw device (rendering model comparison)
    papers/names.ms     — "The Use of Name Spaces in Plan 9"
    papers/9.ms         — "Plan 9 from Bell Labs"
    papers/plumb.ms     — "Plumbing and Other Utilities"
    papers/auth.ms      — "Security in Plan 9"

## What's NOT here

- PostScript/PDF renderings (generate from troff source if needed)
- The full Plan 9 source tree (https://github.com/plan9foundation/plan9)
- Lucida fonts (excluded from the MIT relicense)
- Papers published by USENIX/ACM with separate copyright (link instead)

## Source

Plan 9 from Bell Labs, Fourth Edition. Originally developed at Bell Labs
by Rob Pike, Ken Thompson, Dave Presotto, Phil Winterbottom, and others.
Copyright transferred to the Plan 9 Foundation in March 2021.

## Attribution

Bell Laboratories designed the original system. The Plan 9 Foundation
(https://p9f.org/) maintains the archive and holds the copyright. Both
contributions inform pane's distributed architecture and are credited
in pane's API documentation via `# Plan 9` heritage annotations.
