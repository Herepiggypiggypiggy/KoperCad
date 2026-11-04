# Progress: cabinet start

- 2026-11-04 E0 design and chain merged (#1872).
- 2026-11-04 E1: `novo_layout::spec` (EnclosureSpec, Source, reconcile, catalogue CSV with source and terms). Pure model, no UI yet; no catalogue rows ship (terms not checked). Next: E3 panel, E2 body reader (Rittal thread).
- 2026-11-04 E3: New cabinet window now floats (680 x 600) and lists enclosure cards (your Parts enclosures, your list, shipped catalogue rows: none yet). A pick writes the plate, the `encl.*` record with sources and `depth_mm` in one undo step. Plate size of a Parts enclosure is a default (outer minus 100 x 200 mm), shown as "default" until the plate STEP is read (E4). Not yet: 3D preview, accessories, STEP drop (E4).
