2026-10-25 00:49 UTC Thread started from coordinator relay (round34 idea 2).
2026-10-25 02:00 UTC Engine novo-thermal::whatif and tests (7) pass.
2026-10-25 03:00 UTC Wizard window, 3D preview, apply, commands, menu, 15 app tests pass.
2026-10-25 02:45 UTC Review round 1 (evidence, document_replaced reset, ranking order). Head ec816d4 with fixes; QA 400-402 re-run clean, HEAD.txt in qa/cooling-whatif.
2026-10-25 04:20 UTC #1475 merged. Follow-up PR: only-the-openings wording, window folds to its title bar, cooling advice resets on project replace, QA 403 (layout move preview, new fixture cabinet-whatif-layout.novo) and 404 (folded window). Evidence qa/cooling-whatif-followup/.
2026-10-25 05:30 UTC #1475, #1482, #1483 merged. #1484 (fan note) READY, waiting for merge. Fan sweep (36 cases, 3D vs 2D): 2D reads the fan gain low in 34 of 36, median 1.89x; no single factor, so no calibration. Docs row and section in docs/thermal-validation.md; test validation_fan_gain.rs.
2026-10-25 06:45 UTC #1484 and #1488 merged. 2D engine: fan jet speed term at the device surface (coefficient 3.0, fitted on 600x800x300 mm, 12 cases held out): 3D/2D housing gain 0.69 to 1.28, median 1.00 (was 0.94 to 3.09, median 1.89). FAN_NOTE reworded. QA 400-406 at both sizes in qa/fan-jet-surface/.
