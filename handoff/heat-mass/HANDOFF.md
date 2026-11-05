# Heat H5: handoff

State 2026-11-05: #1859 merged. Branch claude/project-thread-g1mx02 carries PR 2 (shell wiring), made from main (the old branch history was joined with a `-s ours` merge, no force push).
Code: crates/novo-thermal/src/mass.rs (engine, 11 analytic tests), heatup.rs (cabinet network from Scenario::shell(), 15 tests), crates/novo-app/src/app/heat_time.rs (mass_line, mass_hover, shell_note), QA scenarios 18590 (1366x768) and 18591 (1920x1080).
Resume: cargo test -p novo-thermal --lib heatup; cargo test -p novo-app --lib heat_time; scripts/qa.sh --only 1859.
Design: nodes air, devices (one lump), plate, wall stack per face (2 to 6 cells per layer, air gap one cell), door window as a second stack. Conduction and films act over the IEC share b of the face, capacity over the whole face (`Network::add_slab_with`). Default shell reproduces k A_eff exactly (test). "No wall mass" comparison uses mass x 1e-6.
Findings: 1.5 mm steel adds about 12 % to the 90 % time in the test cabinet; the device lump dominates (default 500 J/K per dm2). Insulation or a plate setback change the result more than wall mass.
Traps: do not add films on top of IEC k; never name customer machines; ENCLOSURE_VALIDATED stays false; label stays "model" until H6 passes a measured case; QA scenarios 1602, 380 and 250 short-window were red on main on 2026-11-04.
Not done: wall mass in the animation frames, duty-cycle UI, sun in the transient.
