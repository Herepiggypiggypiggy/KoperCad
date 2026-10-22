# Build mode roadmap

Scope: guided assembly and wiring mode (Pro Panel gap analysis lane 2, C15), local, no server.

Done (PR claude/project-thread-naa4la):
- Engine `novo_commissioning::build`: plan per cabinet, ticks, next/ready/blocked, problem to punch item, change detection by fingerprint, field-file merge.
- Screen `build_mode.rs`: Reports > Build mode (`view.build`), step card, list with lights, Done/Problem/Back, export/import ticks.
- QA scenario 71, tests, docs/BUILD-MODE.md.

Next, in order:
1. DONE (PR show-device): device step on the Cabinet canvas and in 3D, Build mode steps aside behind a bar.
2. Build-mode PDF per cabinet with the ticks printed (the paper twin).
3. Door and wall steps in their own surface coordinates (today door devices say "Fit on the door").
4. Photos on a problem (reuse the commissioning photo path).
5. Per-person claim of a step (two people on one cabinet) merged through the field file.

Open questions: whether wires should be grouped per circuit with a heading, and per-terminal tightening torque once part data has it.
