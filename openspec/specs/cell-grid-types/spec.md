## ADDED Requirements

### Requirement: Cell type
The pane-proto crate SHALL define a `Cell` struct representing a single character cell in the grid. A Cell SHALL contain a character (Unicode scalar value), foreground color, background color, and attribute flags.

#### Scenario: Cell default
- **WHEN** a Cell is created with default values
- **THEN** it SHALL have a space character, default foreground, default background, and no attribute flags set

#### Scenario: Cell with full Unicode
- **WHEN** a Cell is created with a non-BMP Unicode character (e.g., emoji)
- **THEN** the character SHALL be stored and round-trip correctly through serialization

### Requirement: Color type
The pane-proto crate SHALL define a `Color` enum supporting named colors (the standard 16 ANSI colors), indexed colors (0-255, matching xterm-256color), RGB colors (24-bit), and a Default variant indicating the pane's configured default color.

#### Scenario: RGB color round-trip
- **WHEN** a Color::Rgb(r, g, b) is serialized and deserialized
- **THEN** the r, g, b values SHALL be preserved exactly

#### Scenario: Indexed color range
- **WHEN** a Color::Indexed(n) is created with n in 0..=255
- **THEN** it SHALL be valid
- **WHEN** a Color::Indexed value is deserialized with n > 255
- **THEN** deserialization SHALL fail

### Requirement: Cell attributes
The pane-proto crate SHALL define a `CellAttrs` type representing text attributes as a bitflag set. Supported attributes SHALL include: bold, dim, italic, underline, blink, reverse, hidden, and strikethrough.

#### Scenario: Multiple attributes
- **WHEN** CellAttrs is created with bold and italic both set
- **THEN** querying bold SHALL return true, querying italic SHALL return true, and querying underline SHALL return false

#### Scenario: Attribute serialization compactness
- **WHEN** CellAttrs is serialized with postcard
- **THEN** the output SHALL be no larger than 2 bytes

### Requirement: Cell region
The pane-proto crate SHALL define a `CellRegion` struct representing a rectangular region of cells positioned within a pane body. A CellRegion SHALL contain a starting column, starting row, width, and a Vec of Cells (row-major order).

#### Scenario: Region dimensions
- **WHEN** a CellRegion is created with width 10 and 30 cells
- **THEN** it SHALL represent 3 rows of 10 columns

#### Scenario: Empty region
- **WHEN** a CellRegion is created with zero cells
- **THEN** it SHALL be valid and serialize without error

### Requirement: Key event type
The pane-proto crate SHALL define a `KeyEvent` struct containing a key identifier (keysym or character), modifier state (shift, ctrl, alt, super), and press/release state.

#### Scenario: Modified key
- **WHEN** a KeyEvent represents Ctrl+C
- **THEN** the ctrl modifier SHALL be set and the key SHALL identify 'c'

### Requirement: Mouse event type
The pane-proto crate SHALL define a `MouseEvent` struct containing column and row position (cell coordinates, not pixels), button state, modifier state, and event kind (press, release, move, scroll).

#### Scenario: Mouse click in cell grid
- **WHEN** a mouse press occurs at cell column 5, row 3
- **THEN** the MouseEvent SHALL report col=5, row=3
