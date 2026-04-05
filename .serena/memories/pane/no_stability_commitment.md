# No Stability Commitment

pane has no users. The API and architecture are free to change without deprecation, migration paths, or backwards compatibility. There is no commitment to any stability in the API or architecture as a whole.

Consequences:
- Remove dead code outright, don't deprecate
- Rename freely when a better name is found
- Restructure types and traits without shims
- Don't write migration guides or upgrade documentation
- Don't hedge design decisions around "breaking changes" — there are no downstream consumers to break

Lane will inform when this changes.