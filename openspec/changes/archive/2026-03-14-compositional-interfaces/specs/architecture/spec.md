## MODIFIED Requirements

### Requirement: Design pillars
The architecture SHALL include a "Compositional Interfaces" design pillar. Kits SHALL use Rust's native monadic idioms (`Result`/`?`, `Option` combinators, iterator chains) as the primary composition mechanism. Custom domain types with success/failure shape SHALL provide derived combinator APIs. Protocol operation sequences SHALL compose via a combinator builder in pane-app. Observable state SHALL compose via reactive signals in state-oriented kits. Monadic patterns SHALL NOT be forced onto imperative operations where sequential statements are clearer.

#### Scenario: Architecture spec includes compositional interfaces pillar
- **WHEN** the architecture specification is reviewed
- **THEN** it SHALL list "Compositional Interfaces" as a design pillar alongside the existing five pillars

#### Scenario: Kit API review
- **WHEN** a new kit API is designed
- **THEN** it SHALL follow the compositional interfaces principle: derived combinators for domain types, combinator builders for protocol sequences, signals for reactive state, imperative code for mutation
