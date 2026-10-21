# Live view: operate and explain

Scope: make the live (Shift+L) view operable and self-explaining. Source: Creative audit item 4 and Learning creative idea 2.

Done (PR from branch claude/project-thread-z38d09):
- novo-sim `explain`: routes, breaks, causes, plain-words lines, story.
- Live card: Operate list, plain-words summary, Why? mode and Alt+click, break rings, route highlight.

Next, in order:
1. Learning checks reuse `explain` (Learning owner): predict-then-see.
2. (done in the follow-up PR) `sim.why` command.
3. Operate keys (hold keys 1..9 for buttons) and click-through on the symbol itself.
4. Explain for analog quantities (why this current / voltage) from `network::Analysis`.
5. Energy pulses along the route: needs a wire animation layer in novo-canvas; no flow direction is modelled, so only on a route read from the supply.

Open: PLC-forced outputs have no route (reported as "driven").

From the Creative auditor's round 4 (not built yet):
- Operate by holding the mouse on the symbol itself in the drawing, showing a pressed state.
- "Play the story": press S1, wait, release, press S0, with a caption per step ("K1 holds itself in through its own contact 13-14"); the same sequence can open lessons.
- Energy pulses and a motor symbol that changes (no flow direction is modelled, so pulses only on an explained route).
