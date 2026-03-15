## ADDED Requirements

### Requirement: Glyph atlas
The compositor SHALL maintain a GPU texture atlas of rasterized glyphs. The atlas SHALL be populated with ASCII glyphs at startup and extended on demand for other Unicode codepoints. Glyph rasterization SHALL use cosmic-text (or equivalent) for font shaping and rendering.

#### Scenario: ASCII glyphs pre-loaded
- **WHEN** the compositor starts
- **THEN** glyphs for printable ASCII (0x20-0x7E) SHALL be rasterized and loaded into the atlas texture

#### Scenario: Cache miss
- **WHEN** a cell contains a character not yet in the atlas
- **THEN** the glyph SHALL be rasterized on demand and inserted into the atlas

### Requirement: Cell grid rendering
The compositor SHALL render a grid of pane-proto `Cell` values as textured quads using the glyph atlas. Each cell SHALL be rendered with its specified foreground color, background color, and attributes (bold, italic, etc.).

#### Scenario: Colored cells
- **WHEN** a Cell has fg=Red and bg=Blue
- **THEN** the rendered glyph SHALL appear in red on a blue background

#### Scenario: Attribute rendering
- **WHEN** a Cell has the bold attribute set
- **THEN** the rendered glyph SHALL use the bold variant of the font (or synthetic bolding)

#### Scenario: Default colors
- **WHEN** a Cell has fg=Default and bg=Default
- **THEN** the compositor SHALL use the configured default foreground and background colors

### Requirement: Monospace grid layout
Cell grid rendering SHALL use a monospace font. All cells SHALL occupy identical rectangular regions. The grid SHALL be sized in cell units (columns x rows), with pixel dimensions derived from the font metrics.

#### Scenario: Grid alignment
- **WHEN** a CellRegion is rendered
- **THEN** all characters SHALL align to a uniform grid with no fractional-pixel drift between cells

### Requirement: Font loading
The compositor SHALL load a monospace font at startup. The font path or name SHALL be configurable (with a sensible default). Font metrics (cell width, cell height, baseline) SHALL be derived from the loaded font.

#### Scenario: Default font
- **WHEN** the compositor starts without explicit font configuration
- **THEN** it SHALL load a default monospace font and derive cell dimensions from it
