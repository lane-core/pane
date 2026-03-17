## ADDED Requirements

### Requirement: Frutiger Aero visual design
The pane desktop environment SHALL follow a Frutiger Aero visual design philosophy: the refined evolution of 90s desktop design as if Be Inc. had continued development through the early 2000s. Interfaces SHALL be dense, information-rich, and visually polished. The look is opinionated and not themeable.

**Design principles:**
- Power-user information density with refined depth cues
- Controls that look like controls — affordances are visible
- Depth communicates hierarchy, not decoration
- Structure is always visible — you can see where things begin and end

#### Scenario: Visual consistency
- **WHEN** a cell grid pane and a widget pane are displayed side by side
- **THEN** they SHALL share the same visual language (color palette, border treatment, tag line presence) while differing in content model

### Requirement: Depth through lighting
Controls SHALL use subtle vertical gradients (light top, darker bottom) with 1px highlight edges and 1px shadow edges. Depth is communicated through directional lighting, not drop shadows. The effect SHALL be matte and solid, not glossy (not Aqua gel) and not flat (not Metro).

#### Scenario: Button appearance
- **WHEN** a button widget is rendered
- **THEN** it SHALL have a subtle gradient fill, a light top edge, a dark bottom edge, and rounded corners (3-4px radius)

### Requirement: Selective translucency
Floating elements (scratchpad panes, popup choosers) SHALL use translucency to show context — the content beneath is partially visible, communicating "this floats above that." Translucency is applied where it aids comprehension and beauty, not universally.

#### Scenario: Floating pane translucency
- **WHEN** a transient scratchpad pane (e.g., router multi-match chooser) appears
- **THEN** it SHALL be slightly translucent, revealing the pane beneath it

### Requirement: Color palette
The default palette SHALL be warm, saturated, and workspace-like. Warm grey base. Saturated accent colors for focus, dirty state, and active elements. The palette SHALL feel like a well-lit workspace — not a dark cave, not a white void, not a neon playground.

#### Scenario: Focus indication
- **WHEN** a pane gains focus
- **THEN** its border and tag line SHALL shift to a warmer, more saturated accent color distinguishable from unfocused panes

### Requirement: Typography split
Widget pane chrome (tag buttons, widget labels, button text) SHALL use Inter (proportional sans-serif). Cell grid pane content SHALL use Monoid (monospace). Tag line text regions (the editable command area) SHALL use Monoid. This reflects the natural split: proportional for reading, monospace for working.

**Official fonts:**
- **Inter** — proportional UI font (widget labels, button text, proportional chrome)
- **Monoid** — monospace font (cell grids, tag line text regions, code)

#### Scenario: Widget button vs cell grid text
- **WHEN** a widget pane button labeled "Apply" and a cell grid pane showing `cargo build` are both visible
- **THEN** "Apply" SHALL be rendered in Inter and `cargo build` SHALL be rendered in Monoid

### Requirement: Rounded but not bubbly
Interactive controls SHALL have rounded corners with a small radius (3-4px). Enough to feel approachable without losing density. Not sharp 90-degree corners (too stark), not pill-shaped (too soft).

#### Scenario: Control corner radius
- **WHEN** a button, text input, or slider is rendered
- **THEN** its corners SHALL have a consistent small radius, not sharp and not circular

### Requirement: No theme engine
The visual design SHALL be opinionated and fixed. There SHALL NOT be a general-purpose theming engine, theme files, or user-selectable themes at launch. The Frutiger Aero aesthetic IS pane's identity. Future theme support may emerge via filesystem-as-config for individual visual properties (accent color, font size) but not wholesale theme replacement.

#### Scenario: No theme selection UI
- **WHEN** a user looks for a theme selector or theme configuration
- **THEN** none SHALL exist — individual visual properties may be configurable via `/etc/pane/comp/` but not themes as a concept
