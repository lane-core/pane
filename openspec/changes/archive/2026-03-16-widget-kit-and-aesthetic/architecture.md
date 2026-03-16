## Context

Pane currently supports only cell grid content. The project draws from BeOS, which had a full graphical widget toolkit alongside its terminal. The design philosophy — Frutiger Aero, "Be in 2004 with early Aqua influence" — demands polished graphical controls. The cell grid model is one content type, not the only one. The tag line evolves from a raw text string to structured data with dual presentation (text in cell grids, graphical in widget panes).

Reference: BeOS Interface Kit (BButton, BSlider, BListView, etc.), Mac OS X Aqua 1.0 visual refinement, Frutiger Aero design language.

## Goals / Non-Goals

**Goals:**
- Define the WidgetNode tree type and widget event protocol
- Evolve TagLine to structured data with dual rendering
- Establish the Frutiger Aero aesthetic as the opinionated visual identity
- Select taffy (layout), femtovg (rendering), agility (reactive state) as the widget stack
- Design for future tab stacking (data model supports it, implementation deferred)

**Non-Goals:**
- Implementing the widget renderer (that's a future change — this is spec and types only)
- Tab stacking implementation
- Theme engine
- Proportional font integration in the glyph atlas (future — cell grids stay monospace)

## Decisions

### 1. Widget tree is data, not code

Clients build a `WidgetNode` tree (a Value type) and send it to the compositor. The compositor owns layout and rendering. The client never draws. This is the same principle as cell grid content — the compositor renders, the client describes. It's also the BeOS model: BViews described what to draw, app_server drew it.

### 2. taffy for layout, femtovg for rendering

taffy: pure layout computation. Tree in, positions out. No rendering, no windowing, no event loop. Used by Dioxus, Bevy UI, and others. Proven.

femtovg: 2D vector graphics on OpenGL via glow. Canvas API (rounded rects, gradients, text, shadows). Renders into our existing GL context — no separate windowing. This is how we draw beveled Frutiger Aero controls.

### 3. TagLine as structured data

The tag line becomes `TagLine { name, actions, user_actions }` instead of a raw `String`. The protocol carries the structure. The cell grid renderer joins labels into monospace text. The widget renderer draws a graphical tab + buttons. Same data, dual presentation.

Left-click on widget tag buttons works (natural expectation). B2/B3 semantics remain available everywhere for power users. No conflict — left-click is sugar for B2 in tag button context.

### 4. Hybrid panes via CellGrid widget node

A `WidgetNode::CellGrid { region }` allows embedding a cell grid region inside a widget layout. A file manager with a terminal panel at the bottom, an editor with a widget toolbar above the text area — these are hybrid panes using both content models in one body.

### 5. Aesthetic: BeOS density, Aqua refinement

Closer to BeOS than Aqua in density. Smaller controls, tighter spacing. But with Aqua-era rendering polish: subtle gradients, 1px highlight/shadow edges, 3-4px rounded corners, selective translucency on floating elements, warm saturated palette. The sweet spot where every pixel serves information, and every surface has just enough depth to feel real.

## Risks / Trade-offs

**[Widget rendering complexity]** → femtovg + taffy is simpler than a full toolkit, but rendering beveled controls with gradients, rounded corners, and text is still substantial work. Mitigation: start with a small widget set (button, label, text input, slider, checkbox, list, layout boxes) and expand incrementally.

**[Protocol size]** → A complex widget tree serialized via postcard could be large. Mitigation: send diffs (changed subtrees) rather than full trees on each update. Design the protocol for incremental updates from the start.

**[Two rendering paths]** → Cell grid renderer (custom GL quads) and widget renderer (femtovg) are separate code paths. Mitigation: they share the same GL context and frame lifecycle. The compositor dispatches to the right renderer based on PaneKind.

## Open Questions

- Should the WidgetNode tree support custom/extensible widget types (via attrs), or is a fixed enum sufficient for the foreseeable future?
- How does text selection (for B3 routing) work in widget panes? Can you select text across multiple labels? Or only within a single text-bearing widget?
