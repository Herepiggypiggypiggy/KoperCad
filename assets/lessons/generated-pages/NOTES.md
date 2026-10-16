# generated-pages: reviewer notes

Teaches: the pages KoperCAD writes from the drawing (contents, bill of materials, device list, wire list, terminal and cable diagrams) are in the
page list under the folder "Contents and reports", are not drawn by hand, follow the drawing, and are named by section number plus a letter when a
report needs more than one sheet (DESIGN.md section 8).

Project: the pump starter (Gate 1 fixture) polished, with its generated pages (`crates/novo-tests/tests/lessons/documents.rs`). Start = 2 drawn
pages + 6 generated pages (sheets 0a 0b 10 20 30 40 50a 50b). Finished = the same plus an unwired green lamp -P1.

Verification: `crates/novo-app/src/app/lesson_verify_outputs.rs` (`generated_pages_every_step_works_from_its_own_start`, `..._solve_scripts_replay`,
`the_menu_routes_and_the_page_list_click_work`). The first drives every instruction from the start project.

Verified path per step
- open-contents: button "Pages" (app.rs outline_flyout; the list is pinned from 1700 px, then it is open already) and the row "0a  Table of contents"
  under the folder "Contents and reports" (outline.rs, outline_tree.rs). This replaces the old open-outline step, which was true at its start on a wide window.
- read-contents: columns Page, Title, Page group, Panel, Kind (novo-reports toc.rs).
- back-to-drawing: Page Down twice (keymap.rs nav.next_page, workbench.rs); or a click on the row "Control circuit".
- place-lamp: palette place field "lamp green" + click at 104/168 mm (test: `palette_place`, real pointer click); the lamp is `=A1+CAB-P1`.
- open-device-list: Page Down twice (10 Bill of materials, 20 Device list).
- new-row: device list row 5 = lamp, column "Drawn at" `=A1+CAB/2.3` (asserted), pointers y=59.
- sheet-names: sheets 0a 0b 10a 10b 20 30 40 50a 50b after the lamp (asserted).
- what-you-edit: Rename and Delete page are off for generated pages (outline_tree.rs).

Fixed since the first version: open-outline was true on a wide window (now a different first action: open the contents page); the page tree and the
folder "Contents and reports" are named; `{ui:outline.toggle}` rendered as "toggle" (anchor not registered): plain **Pages** now; tag and sheet letters glossed.

GAPS
- Deleting a generated page (turns its kind off, DESIGN section 8): `Tx::delete_page` does it but the menu entry is off ("turned off in the project rules") and
  there is no Rules control; not taught.
- Anchors a pointer would use do not exist (`outline.toggle`, `outline.page.<sheet>`, `palette.field`): the steps point at canvas areas only.
- "Open finished version" (ui.toml) has no button in the app yet; the last step describes the finished result in words instead.
- Wished: a check kind `page_count` with `at_most`, to tick a delete step.

## Pending how
(paste as `how` / `how_nl` when the field lands; keys as {key:command.id})

- intro: (reading, no how)
- open-contents
  - en: ["Click **Pages** at the top left of the drawing (on a window 1700 pixels wide or more the list is already open)", "Under the folder **Contents and reports** click **0a  Table of contents**"]
  - nl: ["Klik linksboven bij de tekening op **Pages** (op een scherm van 1700 pixels of breder staat de lijst al open)", "Klik onder de map **Contents and reports** op **0a  Table of contents**"]
- read-contents: (reading)
- back-to-drawing
  - en: ["Press {key:nav.next_page} (Page Down) once: **1  Main circuit**", "Press {key:nav.next_page} again: **2  Control circuit**", "Or click **Pages**, then the row **Control circuit**"]
  - nl: ["Druk één keer op {key:nav.next_page} (Page Down): **1  Main circuit**", "Druk nogmaals op {key:nav.next_page}: **2  Control circuit**", "Of klik op **Pages** en dan op de regel **Control circuit**"]
- place-lamp
  - en: ["Press {key:tool.place}", "Type **lamp green** and press Enter", "Click the empty spot right of the coil"]
  - nl: ["Druk op {key:tool.place}", "Typ **lamp green** en druk op Enter", "Klik op de lege plek rechts van de spoel"]
- open-device-list
  - en: ["Press {key:nav.next_page} twice: the first press shows **10  Bill of materials**, the second **20  Device list**", "Or click **Pages**, then **20  Device list**"]
  - nl: ["Druk twee keer op {key:nav.next_page}: de eerste keer **10  Bill of materials**, de tweede keer **20  Device list**", "Of klik op **Pages** en dan op **20  Device list**"]
- new-row, sheet-names, what-you-edit: (reading)
