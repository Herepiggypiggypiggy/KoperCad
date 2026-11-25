# Crane export v68b handoff
Branch claude/project-thread-wc1dnp (kopercad). Files: crates/novo-interop/src/eplan/pxf/{plan,model}.rs.
Result on the real export (private, never committed): 331 devices (was 20), 82 strips (was 26), 1744 terminals (was 810), 283 cables (was 62), 3744 wires, 5052 symbols; KoperCAD's nets agree with EPLAN's over 7594 of 7597 points; 0 tag clashes after renaming.
Two bugs fixed: (1) `core_order` is now a total order (the old compare panicked the sort when a cable mixed numeric and text cores); (2) `safe_level` turns characters outside KoperCAD's tag alphabet into `_`, reported as "tags renamed (guessed)".
Resume with the opt-in test: KOPERCAD_CRANE_EPJ=<epj> cargo test -p novo-interop --test crane_real --test crane (all pass on v68b).
Rules: customer export stays on the private mount; tests here are synthetic.
