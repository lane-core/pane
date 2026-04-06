# pane-fs as Unified Query/Namespace System

pane-fs's directory hierarchy is the unification of BFS queries and Plan 9 synthetic filesystems:

- BFS: typed attributes → indices → live queries → dynamic result sets
- Plan 9: synthetic filesystems → computed views → namespace composition
- pane-fs: directory hierarchy IS the query system. Each directory is a view — a projection of pane state through a filter.

`/pane/by-sig/com.pane.agent/` = BFS query `signature == 'com.pane.agent'` expressed as a path.
`/pane/remote/` = `topology == remote`.
Every level of nesting adds a filter. The tree is computed, not stored.

The unified namespace (local + remote interleaved) is the DEFAULT. Filtering to local-only or remote-only is just another computed directory view. Local/remote is one more predicate in the same system as by-sig, by-type, by-user.

The metadata layer (pane-store attributes) provides the DATA. pane-fs provides the PROJECTION. Same system at different tiers.
