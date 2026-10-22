2026-10-14 17:00 UTC  Handoff folder created. #20 at b326a85 (TEXT_FAINT 4.5:1), sent to reviewer. Round-2 layout fixes done; DESIGN-draft ok'd with 3 fixes to workflow. concept.rs and widgets parked in design/code/.
2026-10-14 17:06 UTC  Second reviewer: changes needed on #20 (old chrome in preview, icon names, wording, copper rule). Fixed in 2d60fdd, pushed; both reviewers pinged; PR comment posted.
2026-10-14 17:04 UTC  Workflow opened DESIGN.md as PR #24 (head c7f78ba) with my three corrections; no objection.
2026-10-14 17:06 UTC  Design reviewer: #20 READY on 2d60fdd, frozen (no more pushes). Follow-ups for a new PR from main: CanvasPalette mark entries; issue_pill showing errors and warnings together. CI on 2d60fdd running.
2026-10-14 17:13 UTC  Canvas (WS-03) adding §5 mark fields to CanvasStyle (proposal, warning, error, override_mark, added, modified, removed, mark_text), novo_canvas::Color([u8;4]), alpha in byte, dashing built in. Agreed values: proposal INFO@55%, override #989691, added #7FB086/modified #D6A55C/removed #E0766C @50%, mark_text #C4C2BD. Inter covers U+21B5.
2026-10-14 21:40 UTC  #20 merged (17:25). Branch restarted from main 15c9992. PR #83 opened (3eba511): mark colours in CANVAS_DARK, issue_pill shows both counts. Sent to design reviewer, WS-01, lead.
2026-10-14 21:32 UTC  #83 CI: all three jobs failed instantly with empty output (Actions not starting account-wide). Commented on PR; local checks are the signal.
2026-10-14 21:36 UTC  Design reviewer: #83 READY on 3eba511, frozen. Next follow-up (new PR after #83 merges): snap_marker entry in CanvasPalette, muted/INFO tone instead of copper (app canvas_style_from uses p.selection). WS-01 to switch app pill to widgets::issue_pill.
2026-10-14 21:44 UTC  Reviewer B: #83 READY (correctness) on 3eba511; both reviews READY, waiting for lead merge. Optional note for next PR: comment in issue_pill that infos are deliberately left out.
2026-10-14 21:45 UTC  #83 merged (cdc7bbe). Branch restarted from main. Next PR: CanvasPalette.snap_marker #858380 (app maps it instead of copper selection) + issue_pill comment on infos.
2026-10-14 21:51 UTC  PR #98 opened (f91df63): snap_marker #858380 + issue_pill infos note. CI failed instantly (Actions down); commented. Sent to design reviewer and reviewer B.
2026-10-14 21:53 UTC  Reviewer B: #98 READY on f91df63 (correctness). Frozen; design review pending.
2026-10-14 21:55 UTC  #98 merged. No open layout PRs. Remaining: support WS-01 chrome (Issues outline mode, drawers, inspector, dock) as asked.
2026-10-15 07:35 UTC  Tadjiro asked for name brainstorm (likes Nova star junction and three-phases marks; NovoElectric was picked too quickly). Replied with shortlist: Starpoint (pick), Lodestar, Nodus, Phasor, Triphase, Loom, Knoop/Schakel. Offered availability check (web, GitHub, crates) and lockup mock-up once he picks favourites. Also: wiring mode toggle request (corner-placing vs line-drawing, no overlaps); await 'Two wiring modes' thread proposal and answer it.
2026-10-15 07:45 UTC  Critiqued workflow's wiring-mode proposal (#169 aaaa478): keep one merged draw mode; rename Draw to Manual (Draw is a Ctrl+K category); chip next to Connect in dock (dim when inactive, no copper); mode glyph on icon; W-again flip OK if key repeat ignored; live refusal with NotAllowed cursor, error tint, dashed ghost with x, hint line; 4px click-vs-drag threshold; R flips elbow; acceptance additions. Offered a dock mock-up for #169.
2026-10-15 07:50 UTC  Wiring modes: two competing proposals (#169 workflow Auto|Draw W-again; #168 wiring-modes thread Corners|Lines Shift+W). After reading #168 click table I retracted my 'keep one Draw mode' point: Layout vote = #168 semantics (Corners | Lines), Shift+W, dock chip right of Connect always present (dim when inactive, no copper), no status strip (dock hint instead), refuse live for anything user-placed with NotAllowed cursor + error tint + dashed x ghost + hint; router legs may nudge; diagonals out of v1. Sent to both threads; asked workflow to merge into one spec. Dock mock-up offered, not started.

## 2026-10-15 08:05 UTC
Wiring-mode dock mock-up (chip, dimmed state, refusal look, Compact hint) saved to design/wiring/wiring-dock.png, source design/code/wiring_dock.rs (not committed). Critique sent to workflow (#169) and wiring-modes thread (#168). Next: check merged DESIGN text.

## 2026-10-15 08:35 UTC
Wiring dock mock-up and KoperCAD lockup merged (#174, docs/design/wiring-dock.png, assets/branding/lockup.svg/.png). Decision: product KoperCAD, maker KoperVonk (no clash found on crates.io, GitHub or web; Dutch registers unreachable, so not legal clearance). brand::NAME and installer/launcher/docs strings are in the Release rename PR, not mine (#198 closed). Layout is parked: no proactive work until the lead says otherwise.

## 2026-10-15 09:30 UTC
Merged: #199 (canvas contrast, copper junction token, ring/flag tokens, canvas-feedback mock). Bug 7 (strip label over terminal number) is Canvas #214. Grid dots inside the page: spec sent to Canvas (dots above the sheet fill, minor 1 px, major 2 px, skipped near the frame and in text boxes, hidden under 6/12 px pitch); palette values grid_minor #2E2E33 and grid_major #404045 changed here in a small PR.

## 2026-10-15 15:45 UTC
Merged earlier: #216 (grid palette), #242 (label rule, workflow looks, connected terminals). Cabinet layout started on the lead's task: design doc docs/design/cabinet-layout.md and novo-layout engine (plate, footprints, collisions, clearance) merged as #284/#287; #289 adds rails and the overflow warning plus three doc fixes from design review. Next: ask WS-02 for placement storage and commands, then the place-all planner, then thin Cabinet canvas UI.

## 2026-10-15 19:55 UTC
Cabinet: #372/#378 (canvas and edit), #403 (table face, hover text) merged. #461 ducts READY (4ce992b, ducts centred like rails), #472 table polish READY (2c2274a, merged with main), #444 theme icons for Reports, Commissioning and 3D. Wind-down: handoff written, no new work. Not hands-on tested: the cabinet drag.


## 2026-10-22 Rail band spacing
- (Replaced by the entry below.) Rails are 35 mm bands (`RAIL_BAND_MM`) centred on the rail line, which runs along the top of the row's devices. The planner now keeps the space between rows at least duct + 10 (devices) + 5 (band) + 17.5 (band), and ducts sit clear of the band; `duct_overlaps` tests the band, not the line.
- Rail and device positions are unchanged, so drill templates, cut lists and 3D rails are as before; ducts and row gaps moved (min 57.5 mm with a 25 mm duct, 127.5+ when separated). Open: the first row's band reaches 17.5 mm above the usable area.

## 2026-10-22 Rails run through the device centre
- A rail line is now the middle of the devices on it (horizontal rail: vertical middle of each device; vertical: horizontal middle), not their top edge. The planners put each device at `rail.y - height/2`; the 3D drag snaps a device's centre to a rail line (ducts keep the old rule). Rows at least 35 mm high hide the rail band behind their devices, so the row gap is the plain 40 mm again; a lower row has its band showing, which the planner keeps clear (first row included).
- Drill templates, cut lists and 3D use the rail line unchanged (rail positions move with the rows). Lesson projects regenerated with `panel_lessons`. Older saved plans keep the rail at the device top edge until Place all is run again.
- "Centre rails on devices" (Cabinet, Rails and ducts window, or palette: rails): `novo_layout::centre_rails` puts each DIN rail's line on the middle of the devices mounted on it. Only the rails move (an `update` of the rail, not `MoveRail`, which would carry the devices); one undo step. A rail with devices of different heights follows the tallest one; the shorter ones stay as placed. Ducts and rails without devices are untouched. Moving a rail by a few millimetres is not re-checked against ducts: run the checks or Route all afterwards. QA scenario 87 (`qa/scenarios/87-centre-rails-on-devices.qa`).
