## The Aesthetic Specification

Pane's aesthetic is inseparable from its architecture. The integrated feel comes from the same infrastructure that produces the integrated behavior: every native pane renders through the same kit, the compositor provides uniform chrome, and the result is one coherent visual identity without a central rendering authority.

This spec defines the visual language. The architecture spec (§10) defines how it is realized technically. The foundations spec (§9) defines why it matters philosophically.

---

### Requirement: Frutiger Aero visual design

The pane desktop environment SHALL follow a Frutiger Aero visual design philosophy: the refined evolution of 90s desktop design as if Be Inc. had continued development through the early 2000s. Interfaces SHALL be dense, information-rich, and visually polished. The look is opinionated and not themeable.

**Design principles:**
- Power-user information density with refined depth cues
- Controls that look like controls — affordances are visible
- Depth communicates hierarchy, not decoration
- Structure is always visible — you can see where things begin and end

#### Scenario: Visual consistency across pane types
- **WHEN** a text-content pane and a widget pane are displayed side by side
- **THEN** they SHALL share the same visual language (color palette, border treatment, tag line presence) while differing in content model
- **AND** this consistency SHALL arise from both rendering through the shared kit infrastructure (pane-ui), not from the compositor rendering on their behalf

---

### Requirement: Kit-mediated aesthetic enforcement

Visual consistency SHALL be achieved through the kit, not through centralized rendering. The pane-ui kit encodes the visual language — palette, control styles, depth treatment, spacing, typographic rules — so that any developer using the kit produces output conforming to pane's aesthetic without effort. This is how BeOS achieved its integrated feel: not by centralizing rendering in app_server, but by providing an Interface Kit good enough that everyone used it.

**The rendering split:**
- **Compositor-rendered (chrome):** tag lines, beveled borders, split handles, focus indicators. The compositor owns the chrome — pane's visual identity is consistent across all panes regardless of whether their content is native or legacy.
- **Client-rendered (body):** all body content. Each pane renders its own content into buffers (the Wayland model). The kit provides the rendering infrastructure; the compositor composites the result.

#### Scenario: Native vs. legacy visual coherence
- **WHEN** a pane-native application and a legacy Wayland application are displayed side by side
- **THEN** both SHALL have identical compositor-rendered chrome (borders, tag line, focus treatment)
- **AND** the native application's body content SHALL conform to the pane aesthetic via the kit, while the legacy application's body content renders independently

---

### Requirement: Depth through lighting

Controls SHALL use subtle vertical gradients (light top, darker bottom) with thin highlight edges and thin shadow edges. Depth is communicated through directional lighting, not drop shadows. The effect SHALL be matte and solid, not glossy (not Aqua gel) and not flat (not Metro).

Edge widths, gradient intensities, and other dimensional values are design tokens defined in the kit and scaled appropriately for the output's pixel density. A "thin edge" is one logical pixel at 1x scale, scaled proportionally at higher densities.

#### Scenario: Button appearance
- **WHEN** a button widget is rendered
- **THEN** it SHALL have a subtle gradient fill, a light top edge, a dark bottom edge, and rounded corners at the standard control radius

---

### Requirement: Selective translucency

Floating elements (scratchpad panes, popup choosers) SHALL use translucency to show context — the content beneath is partially visible, communicating "this floats above that." Translucency is applied where it aids comprehension and beauty, not universally.

#### Scenario: Floating pane translucency
- **WHEN** a transient scratchpad pane (e.g., router multi-match chooser) appears
- **THEN** it SHALL be slightly translucent, revealing the pane beneath it

---

### Requirement: Color palette

The default palette SHALL be warm, saturated, and workspace-like. Warm grey base. Saturated accent colors for focus, dirty state, and active elements. The palette SHALL feel like a well-lit workspace — not a dark cave, not a white void, not a neon playground.

Palette values are design tokens defined in the kit and exposed as filesystem-based configuration under `/etc/pane/comp/`. The token vocabulary includes base, surface, chrome, accent, focus, alert, and text color roles.

#### Scenario: Focus indication
- **WHEN** a pane gains focus
- **THEN** its border and tag line SHALL shift to the focus accent color, distinguishable from unfocused panes

---

### Requirement: Typography split

Pane chrome and widget labels SHALL use a proportional sans-serif. Text content (shell output, editor buffers, code, tag line text regions) SHALL use a monospace face. Tag line text regions SHALL use monospace — the tag line is executable text where column alignment matters.

**Official fonts:**
- **Inter** — proportional UI font (widget labels, button text, proportional chrome)
- **Monoid** — monospace font (text content, tag line text regions, code)

Font metrics, sizes, and weights are design tokens defined in the kit and scaled for the output's pixel density via fractional-scale negotiation.

#### Scenario: Widget button vs. text content
- **WHEN** a widget pane button labeled "Apply" and a text pane showing `cargo build` are both visible
- **THEN** "Apply" SHALL be rendered in Inter and `cargo build` SHALL be rendered in Monoid

---

### Requirement: Rounded but not bubbly

Interactive controls SHALL have rounded corners with a small radius — enough to feel approachable without losing density. Not sharp 90-degree corners (too stark), not pill-shaped (too soft). The standard control radius is a design token defined in the kit.

#### Scenario: Control corner radius
- **WHEN** a button, text input, or slider is rendered
- **THEN** its corners SHALL have the standard control radius, not sharp and not circular

---

### Requirement: Design tokens

Dimensional and color values that define the aesthetic — edge widths, corner radii, gradient stops, palette colors, font sizes, spacing units — SHALL be expressed as named design tokens in the kit, not as hardcoded pixel values.

Design tokens serve three purposes:
1. **HiDPI correctness.** Pane supports fractional scaling (wp_fractional_scale_v1). Hardcoded pixel values break at non-integer scale factors. Tokens are defined in logical units and the kit resolves them to physical pixels for each output.
2. **Constrained configurability.** Individual tokens (accent color, font size) are configurable via `/etc/pane/comp/` without exposing a theme engine. The set of configurable tokens is curated — not every token is user-facing.
3. **Rendering backend independence.** The kit renders via Vello (GPU-compute 2D via wgpu). Tokens abstract over rendering details so the aesthetic definition survives backend evolution.

#### Scenario: Fractional scaling
- **WHEN** a pane is displayed on an output with a fractional scale factor (e.g., 1.5x, 1.25x)
- **THEN** all visual elements (edges, radii, spacing, font rendering) SHALL be rendered at the scaled resolution with no blurriness or upscaling artifacts
- **AND** the aesthetic SHALL be visually identical to the 1x presentation, differing only in pixel density

---

### Requirement: GPU-accelerated rendering

The kit SHALL render via Vello — GPU-compute 2D rendering through wgpu. Vector graphics, text, gradients, translucency, and all visual elements of the pane aesthetic are GPU-accelerated. Text rendering uses a shared glyph atlas with instanced drawing; rasterized glyphs are cached and shared across pane-native processes via shared memory.

This is a rendering infrastructure commitment, not an aesthetic choice — but it enables the aesthetic. The depth cues, translucency, and gradient work that define the Frutiger Aero look are computationally tractable only with GPU acceleration.

#### Scenario: Rendering performance
- **WHEN** multiple panes with gradient fills, translucent overlays, and dense text content are visible simultaneously
- **THEN** frame timing SHALL remain smooth, driven by the GPU-compute rendering pipeline

---

### Requirement: No theme engine

The visual design SHALL be opinionated and fixed. There SHALL NOT be a general-purpose theming engine, theme files, or user-selectable themes. The Frutiger Aero aesthetic IS pane's identity.

Individual visual properties (accent color, font size, font choice within the proportional/monospace split) are configurable via filesystem-based configuration under `/etc/pane/comp/`. These are constrained adjustments within the design language, not wholesale theme replacement.

#### Scenario: No theme selection UI
- **WHEN** a user looks for a theme selector or theme configuration
- **THEN** none SHALL exist — individual design tokens may be adjustable via `/etc/pane/comp/` but themes as a concept SHALL NOT exist
