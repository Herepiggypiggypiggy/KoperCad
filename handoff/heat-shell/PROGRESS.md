2026-11-04 07:00 UTC Thread started (coordinator relay). Read heat-research ROADMAP and the 2D/3D wall code.
2026-11-04 07:40 UTC `shell.rs` engine, 2D wiring, tests green (50 lib, 6 shell integration).
2026-11-04 08:10 UTC 3D wiring (`wall_conductances`, plate setback), `shell_view`, `CfdSnapshot::plate_rear_c`; heat closes to 50.2 W of 50 W in the sealed 3D case.
2026-11-04 10:30 UTC #1865 merged (94180b1). HeatSource follow-up from main: Params::sources, lowered at solver entries, 2D and 3D tests.
2026-11-04 11:20 UTC Follow-up to #1894: check_sources before lowered, NaN blockage, doc.
2026-11-04 12:00 UTC #1899 merged. Door frame bridge (DoorEdge) in 2D and 3D, tests.
