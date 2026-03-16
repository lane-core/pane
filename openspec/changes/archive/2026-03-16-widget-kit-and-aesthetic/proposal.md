## Why

The current spec treats the cell grid as the only native content model. This limits pane to text-oriented applications. A desktop environment needs graphical controls — preferences panels, file managers with icon views, media players, system tools. Without a widget model, these either don't exist or are legacy Wayland clients that don't integrate with the tag line, routing, or compositor-rendered chrome.

BeOS had both — terminals and graphical apps shared the same rendering pipeline, the same visual language, the same interaction model. Pane should too.

Additionally, the aesthetic spec ("no gradients, no shadows, no transparency") is too austere. The design philosophy is Frutiger Aero: what if Be Inc. survived into the 2000s and refined their visual design? Dense, information-rich interfaces with polished depth cues. Not flat, not skeumorphic — the sweet spot where 3D depth serves comprehension.

## What Changes

- **PaneKind::Widget** — new pane body type. Clients send a widget tree description; the compositor renders it.
- **WidgetNode tree type** in pane-proto — structured description of widget layouts (buttons, labels, sliders, text inputs, lists, boxes)
- **TagLine as structured data** — name + actions + user actions + text region. Cell grid panes render it as monospace text, widget panes render it as graphical tab + buttons. Same data, different presentation.
- **Aesthetic evolution** — from "90s austere" to "Frutiger Aero: Be in 2004." Subtle gradients for depth, minimal shadows on floating elements, selective translucency, rounded corners (3-4px), proportional fonts in widget chrome, monospace in cell grids. One opinionated look, not themeable.
- **taffy** as layout engine for widget trees (pure computation, no rendering)
- **femtovg** as 2D vector renderer for widget controls (renders into existing glow context)
- **agility** signals for reactive widget state bindings

## Specs Affected

### New
- `widget-kit`: WidgetNode types, widget rendering model, client→compositor widget protocol
- `aesthetic`: Frutiger Aero visual design language, the Be-in-2004 philosophy

### Modified
- `architecture`: PaneKind::Widget, taffy/femtovg deps, tag line as structured data, dual presentation
- `pane-protocol`: TagLine structured type, WidgetNode tree messages, WidgetEvent responses
- `cell-grid-types`: Clarify cell grid as one content model among others (not the only one)

## Impact

- pane-proto gains: TagLine struct, TagAction, WidgetNode enum, WidgetEvent
- pane-comp gains: femtovg renderer for widgets, taffy layout engine, dual tag line rendering
- pane-ui kit gains: widget tree builder API with agility signal bindings
- Aesthetic section of architecture spec rewritten
- Future: tab stacking (tag line data model supports it, implementation deferred)
