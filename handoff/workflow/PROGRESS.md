2026-10-14 16:22 UTC Opened PR #24 (interaction model, design round 1)
2026-10-14 16:40 UTC #24 revised after two critique rounds; reviewer READY; Tab = next proposal (lead)
2026-10-14 16:57 UTC Merged DESIGN.md draft checked by layout thread (3 corrections applied)
2026-10-14 16:59 UTC Lead chose option (b); pushed DESIGN.md onto #24 (c7f78ba), retitled "Design: DESIGN.md"
2026-10-14 17:01 UTC Reviewed WS-06 #38; sent 4 required changes (Enter, menu order, potentials as source, auto-connect preview)
2026-10-14 17:02 UTC Design reviewer: #24 READY on c7f78ba; lead approved; 3 edits + lead's 4 gaps go to a follow-up PR after merge
2026-10-14 17:04 UTC Wrote handoff files
2026-10-14 17:12 UTC Follow-up DESIGN.md edits drafted (handoff/workflow/DESIGN-followup.md); corrected #38 item 4 with WS-06 (facing pins = proposal, 6 px pin radius, drag from pin)
2026-10-14 17:03 UTC #24 merged by the integration lead; DESIGN.md on main
2026-10-14 17:08 UTC Restarted branch from main; opened #43 "Design: DESIGN.md follow-ups" (head f63d0fa); sent SHA to design reviewer and lead
2026-10-14 17:09 UTC Sent DESIGN.md criteria to WS-01, WS-07, WS-08, WS-10, WS-11 and layout (#20 menu/status bar)
2026-10-14 17:06 UTC #43 READY (design reviewer) and merged. Optional nit for a later round: §9 half-sentence on why the empty-canvas New/Open/Recent view is not a start screen
2026-10-14 17:07 UTC WS-08 acknowledged criteria (aliases, Shift+O, ? via Key::Char); layout #20 already has no menu/status bar (READY at 2d60fdd)
2026-10-14 17:10 UTC Checked WS-11 #30 at 800680b: meets DESIGN.md engine criteria; 3 non-blocking notes (Rules doc, localisable strings, enclosure flag later)
2026-10-14 17:11 UTC WS-06 #38 at 09ca513: items 1,2,3,5,6 done; asked for facing pins as post-drop proposals (no Alt), 6 px pin radius, drag-from-pin connect. WS-07 #36 at ea67da2 meets criteria (declined_parts device property OK)
2026-10-14 17:12 UTC WS-10 #31 at cd8482a meets criteria; asked to drop leading slash (3.4 not /3.4) in same-structure refs
2026-10-14 17:12 UTC WS-10 #31 at 2c9f74f: slash dropped; meets criteria, nothing left from workflow
2026-10-14 17:14 UTC WS-01 Workbench plan matches criteria; told them #43 merged so Documents default can go in; awaiting their PR head after #34
2026-10-14 17:18 UTC WS-06 #38 at 904aae0 had all review items (facing pins as proposals, 6 px pin radius, drag-from-pin); #38 later merged by cloud coordinator
2026-10-14 21:25 UTC Resumed after usage limit; read MAIN-HANDOFF §0 and RESUME-2320. Opened #80 (docs): saving (lead decision via WS-01), junction dots 3+ only, letter codes by purpose (MCB F, MPCB Q, rule per class; tester O1), highlighted proposal = Enter target (design-reviewer note 8), §9 start-screen nit. Head e6582ef; sent to design reviewer, lead, WS-01
2026-10-14 21:24 UTC #80 READY on e6582ef (design reviewer). Lead is now cse_01FiLvRBKEcNNHbQEGhYyL1S (sent there). Design queue now owned by session_01NLERJkVHpsVcABrarTR3CF. Nits queued for next design PR: untitled work in %LOCALAPPDATA%\NovoElectric\Unsaved; Ctrl+Shift+S = save a copy
2026-10-14 21:27 UTC #80 merged by lead. Criteria sent: WS-01 (saving, highlight), WS-03 (dots 3+), WS-08 (mpcb=Q, mcb=F), WS-05 (add MPCB symbol, Q), WS-11 (Delete declines via dismiss_suggestion)
2026-10-14 21:28 UTC WS-05: MPCB symbol exists (protection.mpcb, alias mpcb). Told WS-08 to map protection.mpcb/mcp -> Q, mcb* -> F; told lead mpcb3 alias not needed
2026-10-14 21:34 UTC WS-11: Delete -> dismiss_suggestion(tx, wire) in one transact; relayed to WS-01
2026-10-14 21:38 UTC Opened #88 (docs): generated pages reflow in own sections after schematic pages (outputs thread session_01GNLWH7SKcFppZwCxZoC1rw), Unsaved location, Ctrl+Shift+S save a copy. Head 774b5a9; sent to design reviewer session_01NLERJkVHpsVcABrarTR3CF and lead
2026-10-14 21:37 UTC Cloud design reviewer declined #88 (wrapped up); re-sent #88 at 774b5a9 to session_0114emQ5fSFrzcALbR4S18TU
2026-10-14 21:38 UTC #88 READY on 774b5a9; lead told. Nits for next design PR: links into generated sheets point by name, never page.column; 'moves it to' not 'moves it as' (§3 Saving)
2026-10-14 21:38 UTC #88 merged. No open PR of mine.
2026-10-14 21:40 UTC Contactors stay K by default (rule K|Q); opened design PR with #88 nits; answered WS-08
2026-10-14 21:41 UTC Opened #92 (head d89bf2b): contactors K default + K|Q rule, #88 nits, handoff sync; sent to design reviewer, lead, WS-08
2026-10-14 21:40 UTC Lead confirmed contactor K default + Q rule is final; merges #92 on design READY
2026-10-14 21:42 UTC Pushed e431274 to #92: highlighted proposal dashed full strength 2 px (not solid; reviewer's #91 finding). Re-sent to design reviewer; told WS-01
2026-10-14 21:44 UTC #92 merged at e431274; told WS-06, WS-11 (WS-01 earlier) about dashed 2 px highlight. No open PR of mine.
2026-10-14 21:45 UTC WS-06: dashed 2 px highlight committed (5089dde), PR after #91 merges
2026-10-14 21:53 UTC Answered WS-02 on blocks (use BlockDefinition/Instance in Gate 1; project-first storage + libraries; tags allocated, properties/texts substituted, parts proposed). Recording in DESIGN §7 PR
2026-10-14 21:55 UTC Opened #100 (blocks storage/substitution, head 34eca56); sent to design reviewer and lead
2026-10-14 21:55 UTC #100 merged. No open PR of mine.
2026-10-14 22:04 UTC Lead asked: n of m per section (confirmed by design reviewer on #96); opening docs PR
2026-10-14 22:05 UTC Opened #106 (n of m per section, head 9f5e0fb); sent to design reviewer and lead
2026-10-14 22:04 UTC WS-03: junction rule done locally (105d8ff, canvas-drawing; dots only at 3+); PR head to follow
- 2026-10-14: WS-02 confirmed the blocks model (BlockDefinition in project, BlockInstance per placement, per-member overridden fields plus detached/reason); Gate 1 covers place_block and parameter edits; lands after #107, Structure+Dismissal and page freeze.
2026-10-14 22:05 UTC #106 merged. No open PR of mine.
2026-10-14 22:10 UTC Lead asked for non-dotted generated sheet names; opened #108 (20a, 20b ... 20aa; head e90b056); sent to design reviewer, lead, outputs
2026-10-14 22:09 UTC Outputs had proposed 20-1; told them letters (20a) per #108 ('-' is the tag prefix, '/' the structure separator)
2026-10-14 22:10 UTC #108 READY (e90b056). Accepted reviewer's optional note: skip i, l, o in sheet letters; follow-up docs PR after #108 merges; outputs told
2026-10-14 22:10 UTC #108 merged; opening follow-up for skip i/l/o
2026-10-14 22:12 UTC #110 READY (24666e6); lead merges
2026-10-14 22:15 UTC #110 merged. No open PR of mine.
2026-10-14 22:17 UTC WS-02 blocks model checked vs DESIGN §7: matches; flagged that adding a device/connection inside a block must detach too (suggested 'inside' rule)
2026-10-14 22:24 UTC Found novo-editor names inserted pages 1.1 (clashes with page.column); DESIGN PR: page names never contain a dot, inserted pages take letters (3a)
2026-10-15 02:56 UTC WS-02 built 'connection inside block detaches'; suggested end-of-transaction check for a device wired only to members. Re-pinged design reviewer on #116
2026-10-15 02:57 UTC #116 READY (e68a605). Sent WS-06 the insert-before-3a rule (re-letter unfrozen run; frozen names never move)
2026-10-15 02:56 UTC #116 merged. No open PR of mine.
2026-10-15 03:01 UTC WS-02 built the device-inside-block detach rule (follow-ups PR after blocks)
2026-10-15 03:20 UTC Checked WS-06 #133 (page letters, re-letter/frozen, dotted rename refused): matches DESIGN §3; flagged 'Cover a' → 'Cover a a' (should be 'Cover b')
2026-10-15 03:21 UTC WS-06 fixed 'Cover a' → 'Cover b' in #133 (c689e70)
2026-10-15 03:47 UTC Design reviewer asked to record jump/reveal behaviour from WS-03 #144; opening docs PR
2026-10-15 03:50 UTC Opened #148 (jump/reveal behaviour, head ae95b7e); sent to design reviewer and lead
2026-10-15 03:46 UTC #148 READY (ae95b7e); lead merges
2026-10-15 03:47 UTC #148 merged. No open PR of mine.
2026-10-15 07:35 UTC Tadjiro asked for both wiring styles (EPLAN corners, AutoCAD lines), never overlapping, with a toggle + shortcut. Decided Auto + Draw (Draw = corners and lines as one gesture), W again toggles, dock switch, no-overlap rule; DESIGN PR opened, Layout to critique
2026-10-15 07:50 UTC Reconciled with the Two wiring modes thread (PR #168): two modes Corners (default) | Lines, Shift+W toggles (also mid-wire), dock switch, no-overlap enforced in engine (Corners nudge, Lines refuse, reason as data). #169 reworked to carry it; #168 to close after folding its detail; diagonals in DESIGN §12
2026-10-15 07:55 UTC #169 folded in: reviewer items 1-9, Layout chip/refusal feedback, main reviewer (a)(b)(c); wiring thread agreed to close #168
2026-10-15 08:00 UTC #169: dropped wire merging (reviewer: nudge/refuse any wire), nudge preview note; wiring thread cut #168 to engine plan
2026-10-15 08:10 UTC #169: interactive moves are refused like corners (Tadjiro's words), scripts/AI/undo/open accept+report; decision row moved to §8
2026-10-15 08:15 UTC #169 READY at 387f0b3 (design); my re-wrap d325e29 followed, asked reviewer to reconfirm on d325e29. Lead asked #168 as base; asked lead to confirm #169 stays DESIGN PR. No more pushes to #169.
2026-10-15 08:20 UTC Lead confirmed #169 DESIGN PR + #168 engine plan; both need main reviewer READY on current head. Sent heads (#169 d325e29, #168 9754caf) to main reviewer. Feature threads told after both merge.
2026-10-15 08:25 UTC Layout's dock mock is PR #174 (docs/design/wiring-dock.png, a1f6e0a). #169 is READY so no push; link it from DESIGN in a follow-up after #169 and #174 merge, and give it to WS-01.
2026-10-15 08:30 UTC #169 READY from main reviewer on d325e29 too. Follow-up DESIGN PR after merge: end-segment stretch adds a bend if pin leaves axis; R does nothing in Lines; link Layout's mock (#174). Then tell WS-06/03/01/08/02 to build.
2026-10-15 08:35 UTC #169 merged. Follow-up DESIGN PR: end-segment bend and R in Lines.
2026-10-15 08:40 UTC Opened #177 (head 60fbe26); told WS-06/03/01/08/02 and wiring thread to build against merged DESIGN §4.
2026-10-15 08:45 UTC Lead: #169 merged (64415f3), #168 merged (669e0d3). Builders told. Open: #177 (design review).
2026-10-15 08:50 UTC #177 READY from both (60fbe26). Nit for next DESIGN PR: 'at the corner next to the moved pin'; link Layout mock (#174).
2026-10-15 08:53 UTC #177 merged. No open PR of mine. Pending: nit wording + mock link (#174) in a later DESIGN PR; watch engine PRs from the wiring thread against the ten acceptance checks.
2026-10-15 09:00 UTC Opening the last DESIGN follow-up (corner wording, mock link; needs #174 merged first). Then stop per token rules.

## 2026-10-15 UI feedback round (Tadjiro)
- Merged in DESIGN: #204 (right-click menu, place wheel, open ends, pin grab zone, free-start wires), #206 (double-click per tool, drag-move without M, Place key Q), #211 (row placement by drag, link routes incl. rename-to-coil-tag, click on a pin).
- Open: #215 (box selection live highlight, grid dots, + wider / - narrower wording).
- Decisions: grab zone 10 px; junction dot 3.5 px flat; Place = Q (I alias); pitch via wheel or +/- only during a row drag; drag on a wire moves nothing; drag-to-link only when a contact ends over a coil.
- Builders: WS-06 editor, WS-01 shell, WS-03 canvas, WS-08 keymap/link verb, WS-10 xref #212, WS-02 open ends #210.
2026-10-15 15:30 UTC Lead task: revisions in the app on top of novo-revisions, in small PRs from main. DESIGN #247 merged earlier (6556dff).
2026-10-15 15:50 UTC #290 Issue revision window and command `revision.issue` (head a88ec39). Issued gains `changes`; index rule read from the latest index.
2026-10-15 16:05 UTC #298 title block revision fields (head 0833b27): page revision + "(set D)"; TitleRevisions in the canvas, set_revisions on the editor view.
2026-10-15 16:10 UTC #301 DESIGN: 180 ms wheel dwell (timer contact line was already in Shift+L via #281).
2026-10-15 16:30 UTC #305 Changes mode (draft, stacked on #290 and #298): `changes` / `changes B`, marks + dashed ghosts + card.
