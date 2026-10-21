# Scale thread roadmap

Scope: prove KoperCAD stays fast on big projects (audit round 5, blocker 1).

Done: generator, benchmark, cable report netlist fix, 2D heat settle rule (see RESULTS.md).
Next, in order: (1) release-mode budget job on nightly, (2) cached netlist per revision (ask WS-02), (3) 2D heat per-iteration cost, (4) CFD coarse as background job with progress, (5) GPU frame-time scenario in the QA harness at 600 pages, (6) richer generator (varied parts, PLC, multi-level structure, imported-style tags).
Open question: what size is the buyer's biggest project? The generator defaults (600 pages) are a guess.
