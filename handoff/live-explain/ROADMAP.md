# Live view: operate and explain

Scope: make the live (Shift+L) view operable and self-explaining. Source: Creative audit item 4 and Learning creative idea 2.

Done (PR from branch claude/project-thread-z38d09):
- novo-sim `explain`: routes, breaks, causes, plain-words lines, story.
- Live card: Operate list, plain-words summary, Why? mode and Alt+click, break rings, route highlight.

Next, in order:
1. Learning checks reuse `explain` (Learning owner): predict-then-see.
2. A `sim.live.why` command with menu and keymap entry (touches workflow registries).
3. Operate keys (hold keys 1..9 for buttons) and click-through on the symbol itself.
4. Explain for analog quantities (why this current / voltage) from `network::Analysis`.
5. Energy pulses along the route: needs a wire animation layer in novo-canvas; no flow direction is modelled, so only on a route read from the supply.

Open: PLC-forced outputs have no route (reported as "driven").
