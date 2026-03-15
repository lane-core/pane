## MODIFIED Requirements

### Requirement: CellRegion explicit height
CellRegion SHALL include an explicit `height: u16` field. The cells Vec SHALL have exactly `width as usize * height as usize` elements. Construction SHALL validate this invariant.

#### Scenario: Valid region
- **WHEN** a CellRegion is created with width=10, height=3, and 30 cells
- **THEN** it SHALL be valid

#### Scenario: Invalid cell count rejected
- **WHEN** a CellRegion is created with width=10, height=3, and 28 cells
- **THEN** construction SHALL return an error indicating the cell count does not match width * height

### Requirement: Function key range
`NamedKey::F` SHALL accept only values 1-24. Values outside this range SHALL be rejected at construction time via a newtype with `TryFrom<u8>`.

#### Scenario: Valid function key
- **WHEN** a KeyEvent is created with F(12)
- **THEN** it SHALL be valid

#### Scenario: Invalid function key rejected
- **WHEN** code attempts to create F(0) or F(25)
- **THEN** it SHALL fail at the TryFrom conversion
