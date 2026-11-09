# Heat plate map: progress
- 2026-11-09: Plate map, unit test and scenarios 1990 / 1990b.
- 2026-11-09: switched from air layer to the solver's plate sheet field; QA step expect_shots_differ.
- 2026-11-09: follow-up to #1986. Plate map drawn on both faces (visible from behind), default opacity 0.45 with an opacity slider, drawn before the cloud so the cloud shows through. Walls and Door switches (wall maps from `CfdSnapshot::wall_field_c`). Scenario 1991.
