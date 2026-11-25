# Handoff: crane boxes and busbars
Branch claude/project-thread-gbfcuc (kopercad). Files: crates/novo-interop/src/eplan/pxf/{model,read,draw,plan}.rs, crates/novo-editor/src/art.rs, crates/novo-interop/tests/pxf_boxes.rs.
Finding: the export holds each box's size in the placement itself, `S89x5` (`A1651`/`A1652` corners, mm from the insertion point, y up; `A411` 305 black box, 306 PLC box). Symbol graphics are not needed for boxes.
Model: the importer sets symbol properties `box.x0/y0/x1/y1` (mm, y down, from the symbol origin) and `box.dashed`; `art::box_art` draws them; `LibraryArt::cache_key` returns None for such boxes.
Rails: `busbar_rails` in draw.rs joins points `L1.x` inside a frame by potential, level at the insertion height.
Resume on the private export: `KOPERCAD_CRANE_EPJ=<epj> cargo test -p novo-interop`; render: `kopercad-cli import crane <epj> -o x.novo` then `kopercad-cli export pdf x.novo -o x.pdf`; schematic sheet 11 is the busbar page.
Customer files stay private; tests are synthetic (`tests/pxf_boxes.rs`).
