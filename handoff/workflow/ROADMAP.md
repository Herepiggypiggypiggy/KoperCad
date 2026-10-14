# Workflow thread roadmap

Scope: how NovoElectric flows for the worker. Owns `docs/DESIGN.md` (interaction
model, screen, commands, keys, journeys with action budgets, review checklist).
Reviews feature PRs for usability and sends change requests to feature threads
directly. Tadjiro wants its own program, not EPLAN-shaped; be critical; don't wait
on Tadjiro.

## Done
- docs/WORKFLOW.md J1-J13 (#17, #19, merged)
- Interaction model, two critique rounds with layout thread and reviewer
- Merged docs/DESIGN.md onto PR #24 (head c7f78ba); reviewer READY, lead approved
- Review of WS-06 #38: 4 required changes sent to WS-06
- #24 merged; follow-up #43 opened (items a-h below)
- Criteria sent to WS-01, WS-07, WS-08, WS-10, WS-11, layout

## Next, in order
1. DONE: #43 merged. Contents:
   a. auto-connect rule in §2 derived table (touching pins / pin on wire = automatic;
      facing pins at a distance = proposal drawn in the placement preview)
   b. pin vs body drag: pin hit radius 6 screen px, pins win, hover shows connect target (§4)
   c. no-admin default location `<Documents>\NovoElectric\<name>.novo` via known-folder
      lookup (OneDrive redirect) in §6 step 1
   d. page frame and title block: column grid, page.column reference format, revision
      fields; owners WS-10 + canvas (WS-03); frame chosen by a rule
   e. language: Dutch team; localisable command-surface aliases; tags/IEC codes unchanged
   f. "first five minutes" journey with budget (empty-surface hints, ? overlay, dock hint)
   g. mark journey steps that are post-Gate-1 (blocks `dol 4kW x3`, `supply 400V 32A`)
   h. reviewer follow-ups: start-up opens last project directly, Recent when none open;
      inspector flipping -> §12 open question; canvas consumes Tab while focused;
      logical keys for ? and Ctrl+Y on QWERTZ/Dutch layouts
2. Recheck #38 after WS-06 pushes; known gaps: Enter=repeat, W click-click vs
   drag-from-pin, status-bar hint vs dock
3. Send DESIGN.md criteria to WS-06, WS-07, WS-08, WS-10, WS-11; note #3 RESERVED_SHORTCUTS
   needs Ctrl+1..5 (canvases + drawers); #20 preview shows menu bar/status bar (layout)

## Open questions (DESIGN §12)
Dock moving aside; structure board form vs drawing; block storage; light theme.
