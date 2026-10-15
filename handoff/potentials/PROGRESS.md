2026-10-15 12:50 UTC PR A and PR B code done, tests green; opening PRs.
2026-10-15 13:00 UTC #239/#240 merged. Opened PR for pe_override + set_feed_potential (spec: DESIGN #238).
2026-10-15 13:25 UTC #245 merged. First novo-sim piece (live potentials solve) opened as PR.
2026-10-15 14:25 UTC Live potentials view (display side of points 10, 11, 22, 23) built in novo-app `app/live.rs`: Shift+L / `sim.live`, pole badges, lit wires, legend. PR "after build" (lead holds merge until the build is out).
- 2026-10-15 UI thread "Potential and PE menus": right-click on a feed gives the Set potential chips (L1 L2 L3 N PE +24 V 0 V, Other… for project potentials); on terminals Toggle PE terminal (`pe`, check when all PE) and Reset PE override. Model in `novo-editor/src/potential_menu.rs`, commands `terminal.toggle_pe`/`terminal.reset_pe`. PR #270 READY.
2026-10-15 15:00 UTC New… potential and wire override rows built (PR 'Potential New… and wire override'). Next: coil-to-contact coupling in the live view.
