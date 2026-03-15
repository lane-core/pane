## MODIFIED Requirements

### Requirement: Aesthetic — Frutiger Aero
The aesthetic section SHALL be rewritten from "90s-inspired, visible structure" to "Frutiger Aero — the polished evolution of 90s desktop design." The design philosophy: what if Be Inc. survived into the 2000s and refined their visual design alongside the early Aqua era? BeOS's information density and integration, Aqua 1.0's rendering refinement and warmth, combined into a power-user desktop that is both beautiful and dense.

Reference points:
- **BeOS R5 / Haiku** — information density, visible structure, matte bevels, integration
- **Mac OS X 10.0–10.2 (Aqua 1.0)** — rendering quality, subtle translucency, warm saturated palette, polished depth cues
- **Frutiger Aero** — the intersection: 3D depth and organic warmth serving comprehension, not replacing it

One opinionated look. Not themeable. Individual visual properties configurable via filesystem-as-config.

#### Scenario: Aesthetic spec reflects design philosophy
- **WHEN** the architecture spec's aesthetic section is reviewed
- **THEN** it SHALL describe Frutiger Aero with BeOS density and Aqua refinement, not flat design or pure 90s austerity

### Requirement: Dual content model
Pane SHALL support three content models: CellGrid (terminal-style text), Widget (graphical controls rendered by compositor), and Surface (legacy Wayland). Cell grid and widget panes are both first-class native content. The compositor renders both using the same visual language.

#### Scenario: Mixed desktop
- **WHEN** a cell grid pane (shell) and a widget pane (preferences) are tiled side by side
- **THEN** both SHALL have tag lines, borders, and visual treatment from the same design language

### Requirement: Technology — taffy, femtovg, agility
The technology section SHALL include:
- **taffy** — layout engine for widget trees (flexbox/grid computation, no rendering)
- **femtovg** — 2D vector renderer for widget controls (rounded rects, gradients, text; renders into existing glow context)
- **agility** — reactive signals for widget state bindings (candidate, adopted when widget kit is built)

#### Scenario: Widget rendering pipeline
- **WHEN** a widget pane's body is rendered
- **THEN** taffy SHALL compute layout, femtovg SHALL draw controls, both within the same GL frame as cell grid rendering
