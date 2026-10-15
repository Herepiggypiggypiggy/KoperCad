# DXF as a page background: data-model options (WS-24)

Status: decision proposal, written so the entry points could ship first.
Export (File > Export DXF, `kopercad-cli export dxf`) needs none of this.
Import is blocked only on where an imported drawing lives.

## What the designer wants

Drop the mechanical colleague's cabinet or plant drawing under a page, trace
over it, move or hide it, and never have it change the electrical model: no
devices, wires or checks come from it (interop.md section 4). It must travel
with the project file (local-first, no broken link on another machine), stay
small enough that autosave and snapshots stay quick, and undo like any edit.

## Options

| | What | Core change | Cost | Verdict |
| --- | --- | --- | --- | --- |
| A | New `Underlay` entity (page, name, offset, scale, visible, geometry) with commands | New public API in `novo-core` (WS-02), new record kind, JSON and storage migration | Biggest; right long term | Later, once usage shows what it needs |
| B | Page properties only: `underlay.dxf` (the cleaned ASCII text), `underlay.dx`, `underlay.dy`, `underlay.scale`, `underlay.visible` | None: `Page.properties` already holds text, numbers and flags and is saved, undone and diffed today | Smallest; text bloats the page record and snapshots for big drawings | **Default** |
| C | Binary asset table in the project file, page properties hold the asset id | Storage (WS-04) schema migration, core must know assets | Medium; keeps records small | Switch to this if B's size becomes a problem |
| D | Link to the DXF path on disk | None | Breaks when the file moves or the project is shared | Rejected: not local-first |
| E | Convert to native page graphics (lines, arcs, text) the user can edit | New graphics entities | Large; turns a reference into content that is checked and printed | Rejected for now |

## Default: B, with limits that keep it safe

- Stored text is what the reader keeps, not the original file: entities in
  millimetres (`Drawing::scaled_to_mm`), written back with `write_dxf`. It is
  already ASCII, so JSON and SQLite carry it unchanged.
- Hard cap (proposed 2 MB per page); above it the import says so and offers
  to drop hidden layers first. Skipped entity types are counted and shown,
  as designed.
- Everything goes through the existing page-property command, so undo, redo
  and autosave need nothing new.
- The drawing is read once into a `Drawing` and cached per page by hash of
  the property text, so panning never re-parses.
- Rendering (canvas, WS-03) draws it on its own layer under everything, grey,
  never hit-tested or snapped to unless the user turns snapping on. The PDF
  and DXF exporters skip it unless the user asks for the background.
- A later move to A or C keeps the same keys: a migration reads
  `underlay.*` and writes the new record.

## What is not decided

- Whether the background prints by default (proposal: no).
- Per-layer visibility of the imported drawing (proposal: after the first
  version, as a list in the inspector).

## Next steps once B is accepted

1. `novo-interop`: `underlay_from_dxf(bytes) -> Underlay { text, report }`
   (clean, scale to mm, cap).
2. `novo-canvas`: draw the cached drawing as a background layer.
3. `novo-app`: File > Import DXF as background, then Page > Background
   (move, scale, show or hide, remove).
4. `kopercad-cli`: `inspect dxf` for a read-only summary (layers, extents,
   skipped types), usable before importing.
