# Text placement

Status: plan (stage 0 and 1 in progress). Owner: the "Text placement plan" thread.
Companion: the "Text collision stress test" thread supplies the scenarios and the measurement (section 9).

Problem (Tadjiro): texts conflict too often when parts and terminals are placed. A terminal on the left
side of a strip puts its name on top of the strip name, and neighbouring terminals' names run into each
other. The goal is a placement system, not more nudging by hand: every kind of text has a defined home,
a defined fallback order, and the result is deterministic, stable while you edit, and fast on big pages.

## 1. How text is placed today (code review, main `bf37857`)

All positions are in millimetres, in `novo-canvas` (`model.rs`, `declutter.rs`, `note_place.rs`) unless noted.

| Text | Where it is made | Home today | Collision awareness |
| --- | --- | --- | --- |
| Device tag (`-K1`) | `model.rs` `sync_symbol`, `tag_anchor` from the symbol library | Right of the pin box, centred on it, `CenterLeft`, always horizontal; a symbol may give its own anchor | None |
| Terminal name (`1`, `PE`) | `symbol_text` | Same rule as a tag. Terminals sit 5 mm apart, so the right-hand names of a row touch whatever is next to them | None |
| Strip name (`-X1`) | `strip_heads` | Left of the first terminal of a run, and left of any label already on that row | Only against labels of the same item |
| Pin names and numbers, fixed symbol text (`M`, `3~`) | `novo-editor/art.rs` `labels` | Fixed in symbol coordinates, turns with the symbol | None; a rotated or mirrored symbol puts them on the wrong side of a neighbour |
| Cross reference / contact table | `note_place.rs` | Preferred side per `NoteAnchor`, then 6 rings x 4 sides x 5 slides | Yes, against every selectable item, but only against notes placed before it and only at placement time |
| Wire number | `sync_wire` | Middle of the longest segment, above (horizontal) or left (vertical, rotated) | None. Parallel wires 2.5 mm apart stack their numbers on each other; a short segment is shorter than its number |
| Cable tag / core label | `sync_wire` | Under the wire number, 1.2 mm off | None |
| Potential names | `TextContent::Potential` text records | Offset from the wire | None |
| PLC address | `novo-plc` generated texts | Above or below the card, rotated | None |
| Free text and attached text records | `sync_text`, `TextPlacement { offset, rotation, anchor }` | Where the user put it | n/a, and it is already sticky |
| Notes the user moved | `note_offset` | Stays put | n/a |
| On-screen floor | `declutter.rs` | Screen space, per frame: lifts small text to 9 px, drops secondary text that collides | Yes, but only to hide: nothing is moved, and the result changes with zoom |

What goes wrong, in order of how often it shows up:

1. Texts of different owners never see each other. Tag, pin numbers, strip name, wire number and cable tag each
   pick a spot by a local rule. Collisions between owners are the normal case, not the exception.
2. Homes ignore orientation and neighbours. The tag is always on the right; a symbol rotated or mirrored, a terminal
   on the left of a strip, or a symbol whose right side is wired, still gets the right-hand home.
3. Rows of terminals have no row logic. A 5 mm pitch cannot hold a horizontal name that is 8 mm wide.
4. Only notes search for a free spot, and they search once, in an order that depends on symbol order.
5. Hiding at draw time (`declutter`) is the only global rule, so what is visible changes with zoom, and printed
   output (PDF) never gets it.

## 2. Principles

1. One pass, one owner. A single layout pass per page places every text. Nothing else positions text.
2. Defined homes. Each text class has an ordered list of slots per symbol kind, side and orientation. The first slot
   that is free wins. Free space is not searched blindly before the defined slots are used up.
3. Deterministic. The same drawing always gives the same layout, independent of the order things were drawn in.
4. Stable. Layout is a pure function of the drawing (principle 3), so stability means locality: an edit can only change texts inside its dirty region (section 7); everything outside gets exactly the layout a fresh run would give. There is no layout history.
5. Sticky overrides. A text the user moved stays exactly there, and other texts route around it.
6. Honest. When a text cannot be placed cleanly it is shortened (full text on hover) or moved with a leader line.
   It is never silently drawn on top of something else, and a page that still has a conflict says so in the checks.
7. Same result on screen, in the PDF and in print. The layout is in millimetres, independent of zoom. The
   screen-space `declutter` stays only as the last step at very small zoom.

## 3. Model

New module `novo-canvas::textlayout` (pure, no egui), fed by `model.rs` instead of the ad hoc positioning there.
It is a library: Cabinet, 3D and PDF call the same functions with their own obstacles.

```
TextRequest {
    key:       (OwnerId, TextRole),   // stable identity, used for ordering and for sticky overrides
    text:      String,                // full text
    height:    f32,                   // mm
    class:     TextClass,             // priority, section 4
    slots:     Vec<Slot>,             // ordered homes, section 5; empty = free text
    shorten:   Shorten,               // None | Wrap{lines} | Ellipsis{max_chars}
    pinned:    Option<Offset>,        // user override, always wins
    group:     Option<StackGroup>,    // texts that stack together on one side (tag over description)
}
Slot { at: Vec2, anchor: Anchor, rotation: Rot90, stack_dir: Dir }
Placed { key, pos, anchor, rotation, shown_text, leader: Option<Leader>, state: Home | Moved | Shortened | Leader | Conflict }
```

Obstacles, from the scene: symbol ink (with pin leads), wire segments (inflated 0.5 mm), junction dots, page frame and
title block (reserved zone), table and note boxes, and user-pinned texts. They are indexed in the uniform grid
`declutter::Boxes` already uses, so a query costs the cells it touches.

## 4. Priority (who keeps its home when two collide)

Highest first. A text only moves for texts above it.

| Class | Texts |
| --- | --- |
| 0 | User-pinned texts |
| 1 | Device tag, terminal name |
| 2 | Strip name (once per run), PLC address |
| 3 | Pin names and numbers (hug their pin; only the side is chosen) |
| 4 | Cross references and contact tables |
| 5 | Wire numbers, potentials |
| 6 | Cable tag and core labels |
| 7 | Description, annotations, free text that is not pinned |

Inside a class: top to bottom, then left to right, then by id. This is the whole ordering rule.

## 5. Slots

A slot table is data on the symbol kind (library), written for the symbol's unrotated, unmirrored form, then transformed.

* Transform rule. The slot position and the side it names turn and mirror with the symbol. The text itself
  stays horizontal (readable), except texts that are rotated by design (wire numbers on vertical wires, terminal
  names in a tight row). The anchor is re-derived from the side after the transform: the text always grows away
  from the symbol (left side gives `CenterRight`, top gives `BottomCenter`). This is the fix for texts on the
  "wrong side" after R/X.
* Default tables by kind:
  * Two-pole devices (contact, lamp, resistor, coil): tag, then description under the tag on the same side,
    then xref; sides in order right, left, above, below, skipping the sides where a pin lead leaves.
  * Terminals: name outward from the wiring side; in a row with pitch below the text width the name is rotated
    -90 degrees and centred on the terminal. Strip name once at the run start, outside the run, on the side where
    the run's first terminal has no wire. A terminal on the left side of a strip uses the mirrored table, so
    its text goes left and the strip name goes above the run, not onto it.
  * Multi-pin blocks (PLC cards, relays with many pins): pin numbers inside the box edge at each pin, tag above
    the box, description below.
  * Motors, transformers (round or large art): tag and rating around the ink box, in the sides order above.
* Stacking. Texts with the same group on the same side form one stack (tag line, description line, xref line) with a fixed
  line gap of 0.3 times the height, so they never choose separate spots and drift apart.
* Wires. Candidate slots are 1/2, 1/3, 2/3 of each segment, longest first, on the free side of the wire. Parallel wires
  closer than the text height stagger their numbers (alternate side, then alternate along the wire). A segment shorter
  than the text gets no slot; the next segment is used.

## 6. Search, in order

For each request in priority order:

1. Pinned: use it. Done.
2. Home slots in order. A slot is accepted when the text box (plus 0.4 mm air) hits no obstacle or earlier text.
3. Slide: along the side in steps of one text height, +-1, +-2 (the idea `note_place::SLIDES` already has).
4. Rings: 1.5, 3, 5, 8, 12 mm out from the ink (`note_place::RINGS`).
5. Shorten: wrap to up to 3 lines or cut with an ellipsis (description and annotation only; tags, terminal names and
   numbers are never cut) and retry steps 2 to 4 once. Full text is the hover tip and appears in the Inspector.
6. Leader: take the cheapest free spot up to 25 mm away and draw a hairline leader to the home anchor.
7. Conflict: if nothing fits, keep the home, mark the page text-conflict, and add a check problem
   (`text.conflict`, warning) naming the two owners. This is the only way a text may overlap.

Cost, used only to pick among free candidates of the same step: slot rank, plus distance from home, plus 3 per wire it
would run parallel to within 1 mm. Ties: lowest candidate index. No floating-point order dependence: positions are
snapped to 0.05 mm before comparing.

## 7. Stability, overrides, performance

* No hysteresis, by decision. The layout is a pure function of the drawing and its stored user overrides, never of edit history. A text takes the first free slot in the fixed order, so undoing an edit restores the earlier layout, and a saved and reopened file lays out identically. The cost is that a text may move to a better slot when a neighbour is deleted; that is local to the dirty region and is the defined behaviour.
* Dirty region. An edit marks its owner's box grown by 25 mm (the farthest search) dirty. Only requests that touch
  the region are re-solved, in the same global order; the rest reuse the cached result, which equals what a fresh run gives
  because nothing they depend on changed. If a re-solve changes a text near the region's border, the region grows to include its
  neighbours and repeats until nothing outside changes.
* Overrides. Dragging a text stores `TextPlacement.offset` (texts) or the note offset (notes), as today. The
  Inspector and the right-click menu get "Reset text position" (one text) and the Edit menu "Tidy texts on this page"
  (clears all unpinned overrides that were set by a drag, asks first). Overrides are kept through undo.
* Budget. Targets, not yet measured: full page of 2000 texts under 50 ms, incremental edit under 5 ms, to be measured by a benchmark in
  `novo-tests`. Only the open page is laid out; other pages are laid out when opened, when printed and by
  `kopercad-cli check`. A 600-page project never lays out more than one page on the UI thread.
* Setting. Preferences, Drawing: "Text placement" Automatic (default) or "Keep as placed" (never auto-moves
  existing texts, new ones still use the home slots); "Leader lines" on or off. Both are per user, saved with the
  other preferences.

## 8. Reuse beyond the schematic

* Cabinet: device tags on the plate use the same engine with the plate's outlines, ducts and rails as obstacles.
* 3D view: the label layout that exists for heat callouts and device names takes the same stacking and priority rules
  in screen space (it cannot use millimetres); the shared part is the class order and the "full text on hover" rule.
* PDF and print preview: call the layout with the page's millimetres, so print equals screen. The screen-space
  floor is not applied to print.
* Reports that draw tables (terminal screens, cable screens) keep their own grid; they only use the
  shortening rules.

## 9. Test plan (what the testers build)

Two testers, both against the same measurement.

Measurement, added to the scene dump and to `kopercad-cli qa` results (stage 0): per page, `text_overlaps_text`,
`text_overlaps_ink`, `text_overlaps_wire`, `text_outside_page`, `text_leaders`, `text_shortened`, plus the list of colliding
pairs with owners and positions. Targets after stage 5: all four overlap counts 0 on every generated scenario that has
room to be solved; conflicts only where the generator says the page is physically full.

Tester A, "grid of everything" (generated, deterministic, seeded). Cross product, each cell a small page:

* Symbols: every kind in the library (contacts, coils, lamps, motors, breakers, terminals, plugs, PLC cards, sensors),
  4 rotations x mirror x flip, in all 8 neighbour positions of another symbol at distances 2.5, 5, 7.5, 10, 15 mm.
* Text content: short tag, long tag (`=A1+CAB1-K10`), tag with terminal (`-X1:13`), description 1 to 4 lines, pin names,
  xref table 1 to 12 rows, wire numbers (1 to 12 characters), cable tag and core, potential names, PLC address,
  non-Latin characters, empty text.
* Terminals: strip as a row or column, on the left or right of a wire, 2 to 40 terminals at pitch 2.5, 5, 7.5, 10,
  two levels, jumpers, end plates, strip name at start, middle and end, a strip split by another symbol, terminals
  on both sides of one strip.
* Wires: parallel at 2.5 mm for 2 to 12 wires, T and cross junctions, wires shorter than their number, wire through
  a symbol, wire ending at a text, long labelled bus.
* Page edge: everything at 0, 1 and 5 mm from the frame, and under the title block.
* Zoom: layout is zoom independent; render at fit, 100 % and 400 % and check the floor still hides, not moves.

Tester B, "weird and large" (fuzz and real projects):

* Seeded random placement of N = 10, 100, 500, 2000 symbols with random rotation, mirror, pitch and text, 50 seeds,
  asserting the invariants: no overlap unless flagged conflict, layout identical on two runs, identical after a
  save and reopen, identical when symbols are inserted in a different order, and a single edit changes only labels
  within 25 mm.
* Edit storms: move, rotate, delete, undo and redo 200 random edits; the incremental layout after must equal a fresh
  layout of the final drawing exactly (pinned texts are part of the drawing).
* Examples and fixtures: motor starter, the crane import, the AML import, the 600-page scale project; record the
  counts before and after each stage.
* Timing: full-page and incremental timings against the budget in section 7.

Each tester files findings as `audit/text/<n>.md` with the scenario, a screenshot at 1366x768 and 1920x1080, and the
colliding pair. Findings become regression scenarios in `qa/scenarios` and unit tests in the engine.

## 10. Stages (each is a pull request with tests and QA screenshots at 1366x768 and 1920x1080)

| Stage | Content | Where the user sees it |
| --- | --- | --- |
| 0 | Measurement, done: `novo_canvas::text_audit`, the `text_audit` key in the scene dump, and the motor starter ratchet test `text_baseline` (baseline: 0 text on text, 1 text on symbol, 1 text on wire). The stress page generator for Tester A comes with stage 1. Nothing in the UI | Scene dump and QA reports show the counts; nothing moves yet |
| 1 | Engine `textlayout`: obstacles index, slots, search, cost, determinism, dirty-region re-solve, with unit and property tests; not yet wired | Not visible |
| 2 | Devices: tag, description, pin text, symbol-kind slot tables, side-aware transform. Also the `text.conflict` check in the check panel, fed by the text audit of the drawn page; off by default so existing pages do not light up at once | Tags and pin numbers stop colliding when symbols are rotated, mirrored or neighbours; Check panel entry "Text conflicts" (off by default, switch on in the check settings) |
| 3 | Terminals and strips: row logic, rotated names, strip name at run start | The terminal strip case from the report |
| 4 | Wires: wire numbers, cable and core, potentials, parallel stagger | Wire number overlaps gone |
| 5 | Notes move onto the engine, shortening with hover text, leader lines, pinned overrides, Reset and Tidy commands, the setting | Edit menu "Tidy texts", Preferences, Drawing, Text placement |
| 6 | Cabinet, 3D and PDF use the engine | Printed output matches screen |
| 7 | Performance work to the budget and the 600-page scale check | Large projects stay fast |

Honest limits: text boxes are estimates (`ADVANCE` 0.6 per glyph) until the print backend reports real widths;
the engine pads for that. Very dense pages can still need a manual move; those are reported as `text.conflict`, not hidden.
