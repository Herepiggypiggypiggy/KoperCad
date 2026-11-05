# Heat H5: handoff

State 2026-11-04: PR 1 open from branch claude/project-thread-g1mx02 (kopercad): mass engine, cabinet heat-up, strip line. Thread: "Heat H5: thermal mass and heat-up time".
Code: crates/novo-thermal/src/mass.rs (engine, 11 analytic tests), heatup.rs (cabinet network, 9 tests), crates/novo-app/src/app/heat_time.rs (mass_line, mass_hover), model_labels.rs Heat row.
Resume: cargo test -p novo-thermal --lib; cargo test -p novo-app --lib heat_time.
Design: nodes air, devices (one lump), plate, six wall slabs (3 cells). Wall inner film 1.5 k b (as novo-cfd), outer film is the rest so each face equals the IEC k b A; mass changes the time, never the steady total (tested). The "no wall mass" comparison uses mass x 1e-6.
Findings: for sheet steel 1.5 mm the 90 % time grows about 12 % in the test cabinet (wall is only warm part of the way); 4 mm walls grow it much more. Device lump time constant (default 500 J/K per dm2) dominates; check against part data.
Traps: do not add films on top of IEC k; never name customer machines; ENCLOSURE_VALIDATED stays false; label stays "model" until H6 passes a measured case.
H1 contract (ShellModel) not landed at time of writing: wire in after it.
