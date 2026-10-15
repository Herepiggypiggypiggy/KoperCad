2026-10-15 12:50 UTC PR A and PR B code done, tests green; opening PRs.
2026-10-15 13:00 UTC #239/#240 merged. Opened PR for pe_override + set_feed_potential (spec: DESIGN #238).
2026-10-15 13:25 UTC #245 merged. First novo-sim piece (live potentials solve) opened as PR.
2026-10-15 14:25 UTC UI PR #272 for interruption renumbering and the PE override diamond.
2026-10-15 14:25 UTC Live potentials view (display side of points 10, 11, 22, 23) built in novo-app `app/live.rs`: Shift+L / `sim.live`, pole badges, lit wires, legend. PR "after build" (lead holds merge until the build is out).
2026-10-15 14:40 UTC #271 merged. Follow-up: live view repaints only when poles change (no per-frame repaint).
- 2026-10-15 UI thread "Potential and PE menus": right-click on a feed gives the Set potential chips (L1 L2 L3 N PE +24 V 0 V, Other… for project potentials); on terminals Toggle PE terminal (`pe`, check when all PE) and Reset PE override. Model in `novo-editor/src/potential_menu.rs`, commands `terminal.toggle_pe`/`terminal.reset_pe`. PR #270 READY.
2026-10-15 14:55 UTC Merged main into the repaint follow-up (#273); log lines kept from both sides.
2026-10-15 15:00 UTC #272 READY from both reviewers; merged main after #273.
2026-10-15 15:00 UTC New… potential and wire override rows built (PR 'Potential New… and wire override'). Next: coil-to-contact coupling in the live view.
2026-10-15 15:30 UTC Coil to contact coupling built (novo-sim settle + live view coil badges). PR open.
2026-10-15 15:25 UTC Changeover contacts follow their coil (PR 'Live potentials: changeover contacts'). Next: timers.
2026-10-15 16:20 UTC Live potential in wire hover text (PR 'Live potentials: hover a wire').
2026-10-15 16:10 UTC Show where it goes on feeds (PR 'Editor: Show where it goes').
2026-10-15 15:50 UTC Timers (on-delay and off-delay contacts) in the live view (PR 'Live potentials: timers'). Next: Show where it goes, live potential in wire hover.
2026-10-15 15:31 UTC 'Show where it goes' says on hover that it covers this page only. Open: highlight instead of selection.
2026-10-15 15:21 UTC Timer card shows the seconds left; DESIGN Shift+L row mentions timer contacts (follow-up to the timers PR).
2026-10-15 15:42 UTC The inspector has a Time (s) field for a device with a coil or timer contact; it sets the device's delay property (one undo step).
2026-10-15 15:40 UTC Show where it goes now lights the wires and terminals (related highlight) instead of selecting them; it goes out when the selection changes. Next: set a coil timer's delay from the UI.
2026-10-15 17:10 UTC Outputs: lamps, motors, horns, buzzers and valves with two different potentials on their pins get a glow in the live view (novo-sim loads_on). mark_earthed already skips non-Auto PE terminals and seal-in latching settles (tests exist). Next: PLC I/O under simulation.
2026-10-15 16:45 UTC The Time (s) field shows only for a device with a timer contact (a plain relay coil has nothing to time).
2026-10-15 17:15 UTC PLC channels under simulation: a DQ badge switches the output by hand and puts the DC positive potential on its pin; a DI dot fills when its pin carries a potential (docs/design/plc.md 7a). Next: analog channels, a logic model.
2026-10-15 UTC (live-view thread) Context menu on a feed gets "Select where it goes" next to "Show where it goes": same set, but it becomes the real selection (Delete, move and the property sheet act on it).
