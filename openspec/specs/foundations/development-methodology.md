# Pane — Development Methodology

How pane is built, and why the process is inseparable from the design.

---

## The Strategy

Spend careful time thinking about the protocol, the system itself in abstract terms, designing the architecture with explicit lessons drawn from the vetted design philosophy of the 90s systems engineering we reference. To rapidly prototype this system, we use AI coding assistants.

We leverage the fact that modularity and well-chosen abstractions entail that the code surfaces for the most commonly used and critical infrastructural components will be small and well-defined. This allows us to use AI coding assistants to rapidly prototype the system, and pieces can be independently verified, refactored, or outright rewritten as needed — all that is important is that they faithfully implement the specification.

The rapid prototyping allows us to see which design decisions that sounded good initially are actually good in practice, and which ones need to be revised. The modularity and well-chosen abstractions allow us to make changes to the system without having to rewrite large swaths of code, and allow us to quickly iterate on the design until we have a system that is robust, efficient, and easy to maintain. It will also allow us to get an early stage alpha release out the door much sooner.

We are leveraging good design and type systems to constrain the margin of error of our systems. We also get a sense of how much of the resilience of the architecture is due to the design itself, or how much is due to the implementation. We can then use this information to make informed decisions about where to focus our efforts on improving the system.

The hypothesis: **if we get our design right, an AI will be naturally led to make good implementation decisions.** The architecture is the prompt, in the deepest sense. Session types constrain what conversations are valid. Optics constrain how state is accessed. The kit APIs constrain what operations are available. An AI working within these constraints has less room to make mistakes — not because it's smarter, but because the design space has been narrowed to the region where correct implementations live.

## The Meta-Layer: AI as Developer Experience Probe

There is a meta-layer to this strategy. Under the assumption that AI is a fair representation of the average human programmer, when time comes to refine the UX kits, we can use AI coders to study *how easy it is to program within our system.*

This serves two roles simultaneously:

**Live testing of API ergonomics.** We get direct observation of how programs are written against our kits — whether intended semantics emerge sooner or later, concrete bottlenecks from bad API design, where the developer (AI or human) gets confused, where they produce incorrect code that type-checks, where they naturally produce correct code without effort. If an AI can't figure out how to use the Interface Kit correctly, a human developer will struggle too. If an AI naturally produces correct code against our API, we've achieved what the foundations spec calls the NeXTSTEP insight: developer productivity that emerges from the framework's design.

**Validation of the agent infrastructure.** The "ease of use" for agents is a critical component to get right, especially because our AI Kit's strategy for helping users build pane functionality requires that the system be easy to use for agents. If agents building routing rules, translators, and pane modes encounter friction in the extension surface, users asking their agents to customize their experience will encounter the same friction. The agent development experience and the agent-assisted user experience are the same problem. We track this closely as we iterate on the design.

## The Virtuous Cycle

Good design → constrained implementation surface → AI produces correct code faster → faster prototyping → faster feedback on design quality → better design.

The modularity commitment (foundations §5, §6, §7) is what makes this cycle possible. Each component is small enough that an AI can hold its entire specification in context. Each interface is typed enough that incorrect usage fails to compile. Each module can be independently verified, refactored, or rewritten without affecting the rest. The architecture is designed to be built by agents — which means it's designed to be built well.

This is not an accident. The same properties that make pane a good environment for AI agents to inhabit (typed protocols, filesystem-native interfaces, declarative specifications, composable extension) are the properties that make pane a good system for AI agents to build. The design philosophy and the development methodology reinforce each other.
