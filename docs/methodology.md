# Development Methodology

How pane is built: the relationship between human architectural
synthesis and AI implementation, the cross-lineage design discipline,
and the evolutionary development process.

## The heterosis hypothesis

pane treats architectural lineage as genetic material. BeOS and Plan 9
are historically distinct operating system traditions that optimized
for different selection pressures — BeOS for local responsiveness and
media throughput, Plan 9 for location transparency and protocol
uniformity. These optimizations are "incompatible" in the sense that
each tradition's dominant traits mask the other's: BeOS's app_server
was architecturally privileged (the one thing you couldn't treat as
just another networked resource), while Plan 9's local performance
was acceptable for a research lab but never matched Be's desktop
responsiveness.

By combining these divergent lineages under the constraint of session
types and optics, the implementation achieves hybrid vigor — solutions
that express the dominant traits of each tradition while suppressing
their respective weaknesses. The formal methods (session types, optics)
act as the recombination mechanism: they force the implementation to
find the common structure that satisfies both traditions, rather than
allowing retreat into pure BeOS or pure Plan 9 patterns.

When the lineages contradict, the session types often reveal a third
option that transcends the contradiction entirely. When they cannot
be reconciled, the types at minimum guarantee that local departures
remain boundary-safe.

## The dialectic of development

The relationship between human and AI is explicitly dialectical:

**Human role.** Architectural synthesis — recognizing cross-lineage
unifications and setting selection pressures. The key insight that
BeOS live queries and Plan 9 synthetic filesystems are dual
expressions of "namespace as indexed state with materialized views"
is a semantic recognition that no amount of syntactic search would
produce. The human provides the fitness function; the AI explores
the search space.

**AI role.** Rapid prototyping, cross-referencing theoretical
literature against reference implementations (particularly Haiku OS
source), navigating constraints to discover lawful solutions. The AI
handles syntactic recombination — the implementation details — freeing
human working memory for semantic recombination — the pattern
recognition across lineages.

**Caching architecture.** Session types and protocol boundaries serve
as caching layers for implementation knowledge. Once a protocol
contract is established, the human can forget the implementation
details and focus on architectural integration. The AI can reference
the contract when implementing a new subsystem without needing the
full conversation history. This is why `pane-session` was established
first: the foundational contracts must precede the subsystems that
depend on them.

## Evolutionary architecture

Development proceeds as empirical systems design — each subsystem
extension is a controlled experiment in compositional behavior.

**Modularity as genetic isolation.** Session-type boundaries prevent
bad mutations from propagating. A flawed component cannot violate the
protocol contract with its neighbors. This is the biological insight:
strong cell membranes enable complex organisms. Weak boundaries
produce cancerous coupling.

**Testing as phenotypic selection.** CI validates not just local
correctness (genotype) but emergent system properties (phenotype) —
does this component introduce impedance mismatches that ripple through
the protocol graph? The test suite is the selection mechanism.

**Lineage references as phylogenetic constraint.** Checking against
BeOS/Plan 9 conventions maintains lineage purity, preventing drift
toward "GitHub average" solutions that would fracture the system's
coherent personality. The three consulting agents (be-systems-engineer,
plan9-systems-engineer, session-type-consultant) serve as lineage
guardians — each validates that new work is faithful to the tradition
it draws from.

**Sequencing strategy.** `pane-session` (the session-type wire
protocol) was established as the foundational genetic code before any
other subsystems. This sequencing ensures subsequent subsystems are
necessary implications of the session constraints rather than
arbitrary choices. Refactoring risk is bounded — components can evolve
independently without catastrophic disruption.

## Principled pragmatism

When extending functionality, we follow the charter of the original
lineage progenitors (Be Inc., Bell Labs): find the best solution,
temper with practical considerations just as they did when needed,
while striking out boldly with theory and hypothesis when
appropriately motivated.

**Constraint adjudication.** When BeOS and Plan 9 approaches
contradict, the session types often reveal a third option. The
clipboard design is a concrete example: Be's clipboard was
compositor-owned (app_server held the data), Plan 9's was window-
system-owned (/dev/snarf belonged to rio). Neither works for pane's
"host as contingent server" principle. The session-type analysis
revealed a third option: clipboard as an independent service, neither
compositor nor window-system, accessible from any context including
headless instances with no display.

## Compositional extensibility

The system exhibits a satisfying property: each extension of
functionality reveals limitations in other subsystems, but the
coherent design philosophy makes incremental extension tractable.

Extensions function as stress tests that reveal where existing
substrate needs to stretch, but the foundational abstractions (session
types, `/pane/` namespace, optics) ensure these are local deformations
rather than structural fractures. Because session types guarantee
cross-boundary communication remains well-formed, subsystems can be
refactored without breaking dependents.

This enables distributed development — both across human/AI
collaboration and across parallel agent workstreams. The protocol
contract is the coordination mechanism: independent agents working
on independent subsystems cannot produce incompatible implementations
because the session types won't let them.

## Licensing as lineage policy

The dual licensing reflects the biological metaphor:

**BSD (protocol and kits).** The germ line remains maximally free.
Anyone can hybridize pane's session types and kit APIs with their own
lineage without friction, preserving genetic diversity. The protocol
types are the most valuable thing pane produces — they should
propagate as widely as possible.

**AGPL (servers).** Somatic expressions — specific instantiations of
the protocol as running server processes — commit to the commons.
Running servers must share improvements back, preventing enclosure of
the phenotype while keeping the genotype unrestricted.

This ensures that recombinant freedom (protocol diversity) is
maximized while ecosystem contributions (running implementations)
flow back to the commons.
