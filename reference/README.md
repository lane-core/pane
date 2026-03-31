reference
=========

reference material hosted in-repo for portability and agent access.

haiku-book
----------

the Haiku Book — API reference documentation for the Haiku operating
system, the open-source continuation of BeOS. copied from
[haiku/haiku](https://github.com/haiku/haiku) `docs/user/`.

pane's kit API descends from the BeOS/Haiku programming model. this
local copy ensures the reference material is available to any agent
or contributor working in the repo without depending on external
paths or network access.

MIT licensed by the Haiku project. see `haiku-book/LICENSE`.

### generating HTML

    cd reference/haiku-book
    doxygen Doxyfile

output goes to `../../generated/doxygen/html/`.

### key files for pane development

    app/BApplication.dox    — App lineage
    app/BHandler.dox        — Handler lineage
    app/BLooper.dox         — looper/threading model
    app/BMessenger.dox      — Messenger lineage
    app/Message.dox         — Message lineage
    app/MessageFilter.dox   — MessageFilter lineage
    interface/Window.dox    — Pane (BWindow) lineage

### attribution

Be, Inc. designed the original API. the Haiku project
(https://www.haiku-os.org/) spent 25 years extending, refining,
and documenting it. both contributions are invaluable to pane's
design and are credited in pane's API documentation.

plan9
-----

selected documentation from Plan 9 from Bell Labs — man pages and
papers that informed pane's distributed architecture, protocol
design, and namespace model. see `plan9/README.md` for details.

MIT licensed by the Plan 9 Foundation. see `plan9/LICENSE`.
