# generated-pages: reviewer notes

Teaches: the pages KoperCAD writes from the drawing (contents, bill of materials, device list, wire list,
terminal and cable diagrams) are in the outline after the drawn pages, are not drawn by hand, follow the drawing,
and are named by section number plus a letter when a report needs more than one sheet (DESIGN.md section 8).

Project: the pump starter (Gate 1 fixture, same builder as Open example) polished and with its generated pages
(`crates/novo-tests/tests/lessons/documents.rs`). Start = 2 drawn pages + 6 generated pages (sheets 0a 0b 10 20 30 40 50a 50b).
Finished = the same plus an unwired green lamp -P1 (placed with `novo_editor::edit::place`), which turns the bill of
materials into 10a + 10b. Tests assert every claim the steps make (rows, sheet names, row positions of the pointers).

Verified UI path per step (details are `# verified` comments in lesson.toml)
- open-outline: flyout button "Pages" app.rs:1529-1545 (no tooltip); pinned from 1700 px, app.rs:110, 857; Pin button app.rs:1015-1025.
- spot-generated / open-contents / sheet-names: rows `<sheet>  <title> · <section>` backend.rs:700-742; click opens the sheet app.rs:1113-1117.
- read-contents: columns Page, Title, Page group, Panel, Kind (novo-reports toc.rs:166-170); sheet drawn in the frame reports.rs:78-107.
- back-to-drawing, open-device-list: PageDown nav.next_page keymap.rs:385, workbench.rs:480-487 (steps pages_in_order: 0, 1, 2, 10, 20 ...).
- place-lamp: Q tool.place keymap.rs:344, field in Place mode workbench.rs:448 + command_field.rs:185, dock tooltip dock.rs:144.
- new-row: devices.rs:41-50 columns; live update reports.rs:45-52.
- what-you-edit: rename only for drawn pages app.rs:1454-1466.

GAPS / oddities
- No GAP blocks this lesson. Not taught on purpose: deleting a generated page (DESIGN section 8 says it turns the kind off and Rules turns
  it back on): the app has no page delete and no such rule yet.
- The step open-outline is already true on a window of 1700 px or more (outline pinned), so it ticks at once.
- The unwired lamp raises an UnconnectedPin warning (the pill); wiring is another lesson.
- Not verified: the pointer `outline.page.<sheet>` ids (dynamic per row) are a proposal for the anchor registry.
