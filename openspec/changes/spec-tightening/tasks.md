## 1. Research: BeOS/Haiku

- [ ] 1.1 Read the Haiku API docs comprehensively: Application Kit, Interface Kit, Storage Kit, Support Kit, Translation Kit
- [ ] 1.2 Understand the BMessage/BLooper/BHandler threading model — why it produced stability
- [ ] 1.3 Understand the translation kit plugin model — how extensibility worked in practice
- [ ] 1.4 Understand BFS attributes and BQuery — how metadata-as-first-class changed the UX
- [ ] 1.5 Understand the replicant/BArchivable system — what it enabled and what it cost
- [ ] 1.6 Understand BRoster and launch_daemon — the evolution from tracker to supervisor
- [ ] 1.7 How do these ideas inform pane's systems design? Where does pane draw from BeOS/Haiku, where does it diverge, and why?

## 2. Research: Plan 9

- [ ] 2.1 Read Pike's rio paper and the acme paper
- [ ] 2.2 Understand per-process namespaces — how bind/mount created composability
- [ ] 2.3 Understand the plumber — the rule format, the message protocol, how it was used
- [ ] 2.4 Understand 9P — what it meant to have one protocol for everything
- [ ] 2.5 Understand Plan 9's distributed network architecture — how `cpu`, `import`, and namespace export made the network transparent, how a terminal could use a remote CPU seamlessly, what this implies for pane in a networked context
- [ ] 2.6 Understand how acme's filesystem interface worked in practice
- [ ] 2.7 How do these ideas inform pane's systems design? Where does pane draw from Plan 9, where does it diverge, and why?

## 3. Research: Session types

- [ ] 3.1 Read Vasconcelos "Fundamentals of Session Types" — understand the theory from foundations
- [ ] 3.2 Understand the Caires-Pfenning correspondence (linear logic ↔ session types)
- [ ] 3.3 Understand how `par` implements session types in Rust — read the source, understand the design choices
- [ ] 3.4 Understand what deadlock freedom guarantees and what it costs
- [ ] 3.5 How does committing to `par` and session types inform pane's systems design? What does it enable, what does it constrain, what design possibilities open up?

## 4. Architecture spec rewrite

- [ ] 4.1 Rewrite Vision grounded in research
- [ ] 4.2 Rewrite all design pillars for accuracy and consistency
- [ ] 4.3 Update all server descriptions
- [ ] 4.4 Update all kit descriptions
- [ ] 4.5 Update protocol section
- [ ] 4.6 Update composition examples
- [ ] 4.7 Resolve all contradictions
- [ ] 4.8 Review Technology and Build Sequence

## 5. Sync and audit

- [ ] 5.1 Sync pane-shell, pane-route, pane-roster specs to main
- [ ] 5.2 Audit all specs for stale references
- [ ] 5.3 Review README for accuracy and consistency
- [ ] 5.4 Final end-to-end read of everything
