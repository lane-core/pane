# Haiku Book Reference

The Haiku Book (Haiku's API reference documentation) is hosted in-repo at `reference/haiku-book/`.

- 273 Doxygen `.dox` source files, 3.7 MB
- MIT licensed — copied from `haiku/haiku` `docs/user/`
- Generate HTML: `cd reference/haiku-book && doxygen Doxyfile`

## Key files for pane development

- `app/BApplication.dox` — App lineage
- `app/BHandler.dox` — Handler lineage
- `app/BLooper.dox` — looper/threading model
- `app/BMessenger.dox` — Messenger lineage
- `app/Message.dox` — Message lineage
- `app/MessageFilter.dox` — MessageFilter lineage
- `interface/Window.dox` — Pane (BWindow) lineage
- `storage/NodeMonitor.dox` — pane-notify lineage

## Usage

When consulting the be-systems-engineer agent or writing heritage annotations in doc comments, reference specific `.dox` files from this local copy rather than relying on external sources.

## Documentation style guide

`docs/kit-documentation-style.md` governs how pane's API docs are written, including `# BeOS` heritage annotations that credit both Be and Haiku.
