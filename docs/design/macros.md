# Macros: reusable circuits in a per-user library

Owner: Macros thread. Status: design, nothing implemented yet. Builds on `Tx::define_block` and
`Tx::place_block` in `novo-core` (DESIGN section 7); no new model records are needed for step 1.

## Goal

Select a motor starter (breaker, contactor, overload, terminals), save it once as a macro, and drop it
on any page of any project: tags and wire numbers come out right, the user sees a preview before
placing, and parts and cabinet positions can come along. Macros live in a library on the user's
device and can be exported to and imported from a single file, so a team can share them.

## Decisions

1. **A macro is a block definition.** Saving a macro runs `define_block` on the selection; placing one
   runs `place_block`. The placed result is a linked block instance, so DESIGN's rule against frozen
   macros holds: parameter edits and library updates behave as for any block. The user word is
   "macro"; the model word stays "block".
2. **The library is a folder of files**, one macro per file, in the user's profile and never anywhere
   needing admin rights: `%APPDATA%\KoperCAD\Macros\` on Windows and `<platform data dir>/KoperCAD/Macros/` elsewhere,
   built from `DATA_DIR_NAME` like `keymap.txt` and the user parts catalog (the roaming data folder was
   renamed to KoperCAD and is migrated at start, `migrate.rs`). DESIGN section 7 named the old
   `%APPDATA%\NovoElectric\Blocks`; this PR updates that line, so there is one data folder. An optional company folder the user
   points at is read the same way.
3. **One file format for library and sharing.** `<name>.kcmacro` is UTF-8 JSON (diffable, no
   binary). Export of several macros is the same file with a list; import accepts either.
4. **Macros are copies, like any library source.** Placing one into a project copies the definition into
   the project (so a `.novo` file stays complete) and records `BlockSource` (`library` = the folder-relative path, which names the macro; `version` = the macro's `revision`);
   a newer library version is offered as an update, never applied silently.

## File format (version 1)

Agreed with the Macro library thread, which owns the library and file code:

```
{ "format": "kopercad-macro", "version": 1, "revision": 1,
  "name": "dol-starter", "description": "...", "author": "...", "tags": ["motor"],
  "definition": <novo_core::BlockDefinition as serialised, parameters and members included> }
```

No new core types. When a macro is copied into a project the library fills `definition.source`
(`BlockSource`: `library` = folder-relative path, `version` = `revision`). Later, optional additive
fields carry the extras of step 5 (`parts`: manufacturer and order number per template device;
`cabinet`: offsets per template device). Readers ignore unknown fields and keep them on re-export; a
newer `version` is refused with the file untouched; a file that fails validation is rejected whole.

## Who owns what

| Piece | Owner | Where |
| --- | --- | --- |
| File read/write, per-user folder (subfolders are folders), optional company folder, index and search, rename, delete, move, import and export, thumbnails, starter macros | Macro library thread | module `macros` in `novo-parts` (no new crate) |
| Capture (`DefineBlock`), placement, renumbering, wire numbers, parameters, preview summary, parts and cabinet carry-over | This thread | `novo-workflow` helpers over `novo-core` commands |
| Ghost, browser drawer, save and import dialogs, commands | Whoever lands first, kept thin | `novo-app` |

## What a placement does

- **Tags.** Already handled by `place_block`: each device keeps its letter code and takes the next
  free number from the numbering rule in the target page's structure. Placing three starters gives
  three distinct sets; no tag from the source project survives.
- **Wire numbers.** The project has no wire numbering engine yet, so a macro's numbers are cleared on
  placement by default and the wires stay unnumbered until one numbers them. A macro can opt in to
  keeping its numbers (for a vendor panel with fixed labels); a number another wire already carries is
  dropped and reported as a clash in the preview, never kept twice. Implemented in
  `novo_workflow::macro_place` (`WireNumbers::Clear` / `Keep`).
- **Potentials, feeds, interruptions, terminals.** As for blocks: the first three cannot be part of a
  macro (they join nets across the project). Terminals are allowed in a macro as devices in a strip
  (the same-name join rule then applies in the target project); this needs a core check and is the last step
  (step 6).
- **Parts.** A part id is project-scoped, so the macro file keeps the part itself next to the id of the
  device in the definition (`parts`: manufacturer, order number, type number, description). On
  placement the project's part with that manufacturer and order number is used, or one is added when
  the project has none, and it is assigned to the placed device. So a macro works in a project whose
  parts list is empty, and the same part is added once however often the macro is placed. Carrying
  parts is a checkbox in the Place window, on by default when the macro has parts. Implemented in
  `novo_workflow::macro_place` (`PartRef`, `captured_parts`, `PlaceOptions::parts`).
- **Cabinet placement.** Stored as offsets relative to the macro's first device on its plate. On
  placement, if the target structure has a plate, the devices are placed at the offsets through the
  layout engine's overlap and rail rules; whatever does not fit lands in the Unplaced list. Off by
  default, since a different cabinet usually has a different arrangement.
- **One undo step.** Placement, with its parts and cabinet positions, is a single command batch.

## Preview before placing

The macro follows the pointer as a ghost on the page (no model change), with a one-line summary: the
tags it will take ("-Q4, -K4, -F4, -X5:1..6"), wire numbers it will take, parts it cannot find, and
anything off the page. Click or Enter places it; Esc cancels; R turns it before placing (as for any
symbol). Parameters are asked in the inspector while the ghost is shown.

## Browser

Command surface: `macro` opens the browser as a drawer, `save macro` takes the current selection.
Rows show name, description, device count, source folder, and the thumbnail. Actions: place, rename,
delete, edit description, export, import. Rename is a file rename plus the name inside; delete asks
once and moves the file to a `.trash` subfolder of the library (recoverable, never silently
destroyed). Search filters by name and description; recently used macros sort first.

## Saving a macro

`save macro` needs a selection that `define_block` accepts, and asks only for a name (prefilled from
the main device, "dol-starter"). It lists what it left out and why (wires to objects outside the
selection, terminals until step 6). Saving over a name asks first and bumps `revision`.

## Where the code goes

See "Who owns what". If a model gap shows up (for example the terminal case), it is described in the
pull request and sent to WS-02, not patched here.

## Steps (each a small draft pull request)

1. This note.
2. Macro file and library folder (Macro library thread, `novo-parts`).
3. Placement helper: block placement plus wire numbers and the preview summary.
4. Parts and cabinet offsets on save and placement.
5. App: `save macro`, browser drawer, ghost preview, import and export menu items.
6. Terminals inside macros (needs WS-02 review), update from library.

## Not in this design

Cloud or account-based sharing (a file and a shared folder are enough), macro marketplaces, and
editing a macro's contents in place (open it by placing it, change it, and save over the name).
