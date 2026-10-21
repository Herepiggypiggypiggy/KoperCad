# Responsiveness and background jobs: roadmap

Scope: the app never feels frozen on a big project (continues handoff/scale).

Done (PR "Responsiveness", branch claude/project-thread-p4a5ju):
1. `Project::netlist()` is built once per project revision and shared (`Arc<Netlist>`); callers that keep one hold the Arc.
2. Live view: the Operate list and the "the buttons are on page X" pointer are cached per revision (the pointer scanned every page, each with its own rest state, every frame).
3. Design checks: on projects over 1,500 symbols + wires they run on a worker thread; the last finished answer stays on screen, the pill says "updating..." (or "Checking..." before the first answer). Exports and reports still wait for the current answer.
4. Machine options (#1211): `options::evaluate` no longer clones every record twice per pass; stops early when nothing is switched off.
5. Heat: checked, not changed. 2D heat and 3D CFD already run on a worker thread with cancel (`heat_run.rs`, scheduler in novo-cfd).
6. QA: control command and scenario step `frame_stats`, scenario `40-scale-frames.qa` (slow), report section "Measurements"; scale_bench `--save-project`.

Next, in order: see HANDOFF.md "Not done".
