# T3: 3D view flags: roadmap

Scope (LANE-PROMPTS.md T3): draw flags in the 3D view at projected anchors, constant size, faded when hidden, on top when selected, keyboard reachable; 3D editing refusals become transient yellow flags; heat label and duct fill get a triangle cue (tint and percentage stay).

Done in this PR: `view3d_flags.rs` (shared painter API), `View3d::hidden_behind`, store flags drawn in 3D, popover by click or Enter, Locate selects the anchored parts, refusals (duct over duct, no room for a copy, rail or duct typed values), triangle cue on heat labels, duct fill (with the percentage) and drag readouts, census ceiling `crates/novo-app/census/view3d.txt` 12 to 9.

Next (other lanes): T2, T4, T6 call `view3d_flags::paint` through the flag store, nothing more to wire. T4 turns the duct fill cue into store flags. T1 retires the old in-view item marks (the black circle with a cross) when the Cabinet problem list goes.

Open: wire anchors need an `extra` spot from T4 (3D wire runs are routes, not items).
