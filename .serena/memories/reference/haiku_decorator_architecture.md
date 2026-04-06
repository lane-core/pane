# Haiku Decorator Architecture Reference

## Class hierarchy

- `Decorator` (abstract, /src/servers/app/decorator/Decorator.h) — owns tabs, border rects, footprint, hit testing
- `TabDecorator` (TabDecorator.h) — adds tab layout, bevel colors, frame drawing abstractions
- `DefaultDecorator` (DefaultDecorator.cpp) — the yellow-tabbed look, draws via DrawingEngine

## Key mechanisms

- **Footprint:** GetFootprint() returns BRegion of all chrome area. Content region = frame rect minus footprint.
- **Hit testing:** RegionAt(BPoint) returns which region was clicked (REGION_TAB, REGION_CLOSE_BUTTON, etc.)
- **Chrome invalidation:** Window::_DrawBorder() clips dirty region to border area, draws decorator, copies to front buffer
- **Content invalidation:** Window::_TriggerContentRedraw() handles exposed area background clearing, queues update session for client
- **Update session:** Two sessions (current/pending). Server sends _UPDATE_ to client, client does BeginUpdate/Draw/EndUpdate cycle.

## Rendering threading flow

1. Desktop thread marks dirty regions, sends RequestRedraw to ServerWindow
2. ServerWindow thread: _DrawBorder() (server-side chrome), _TriggerContentRedraw() (clears backgrounds, queues client update)
3. Client receives _UPDATE_, calls BeginUpdate, executes BView::Draw(), calls EndUpdate
4. EndUpdate copies dirty content to front buffer

## Mapping to pane

- Decorator → PaneDecorator trait (footprint, content_rect, hit_test, render_chrome)
- No shared framebuffer — compositor renders chrome + composites client content
- No drawing protocol — clients send SetContent, compositor composites
- No server-side view tree — clients own their content rendering
- Glyph atlas is for compositor chrome text only