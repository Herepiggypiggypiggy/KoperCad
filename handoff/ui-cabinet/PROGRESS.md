2026-10-15 19:00 UTC Inspector (plate size, device data, problems) built, tests pass; checklist written.
2026-10-15 18:50 UTC #443 rebased on #403 (84a365c); owners aligned: one right panel, sections from #409 and #419; checklist extended with 3D look and part fields rows.
2026-10-15 19:05 UTC Review fixes: inspector is now an overlay (276 px, away from selection), plate card behind a Plate toggle, merged main, theme field/section_label, problem wording shared with #403.
2026-10-15 19:10 UTC #443 merged (077a510). Branch restarted from main. PR 2: problems list in the Cabinet header.
2026-10-15 19:08 UTC 3D face: wire after #481 (view3d widget) and #391 (scene build) merge; calls in 3D thread message: widget::install once, View3d::new per face, show(ui,&scene,&library,&look,opts), cabinet.view3d command, view.fit.
