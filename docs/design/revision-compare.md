# Revision tracking: compare any two states, readable change list

Status: proposal for review (Revision compare thread). No code changes with
this document. It extends DESIGN.md §7 "Revisions" and does not change it.

## What is already there

DESIGN §7 and WS-18 already give a named revision: `Revision` (index, date,
author, one-line description, `properties`), one undo step to issue it, a
compressed snapshot of the whole project per revision in the `.novo` file
(`snapshots` table, dropped on open when the revision was undone), the
index rule, page freezes, `novo_revisions::diff` (id-matched
added/removed/changed by page), `changed_sheets` for generated sheets,
`PageRevisions` for the title block, the Issue revision window, Changes mode
with change marks and dashed ghosts of removed objects (always "since a
revision, against now"), and the page revision fields in the title block.
The revision table above the title block is the draft #343.

Nothing is stored outside the project file, nothing needs admin rights, and
issuing goes through a command, so undo and autosave already hold.

## What is missing

1. Changes only compares a revision with now. Comparing A with C, or
   C with B, is not possible.
2. The result is a list of field names ("Changed since B: tag, rotation").
   A designer wants to read it: which device, what it was and what it is
   now, grouped by kind.
3. The comparison exists only on the canvas. It cannot be listed, searched,
   printed, exported or run from the command line and scripts.
4. A revision has one line of text. Longer notes ("why") have no place.
5. The revision table (#343) is a draft and not yet tied to the above.

## Decisions

**A revision stays an issued revision.** No second kind of "snapshot" is
added. Saving is not a checkpoint (§3, §8) and a silent snapshot kind would
bring that confusion back. "Named" is the index plus the description;
the author and date are already stored.

**Longer note.** Stored as `Revision.properties["note"]`, so `novo-core`
needs no API change. The Issue revision window gets an optional second
field. The revision table prints the one-line description; the note shows in
the compare list and the report.

**Compare any two states.** A side is `Revision(id)` or `Now`. The default
stays latest revision against now. Any pair is allowed, in either order
(the older one is always the "from" side internally; the labels follow what
the user picked). Revisions without a kept snapshot (opened from a file that
lost it) are listed greyed with the reason, and never offered.

## Engine (`novo-revisions`)

```text
Side        = Revision(RevisionId) | Now
Comparison  { from: Side, to: Side, changes: ChangeSet,
              sheets: Vec<SheetChange>, entries: Vec<Entry> }
Entry       { id: AnyId, group: Group, label: String, kind: Added|Removed|Changed,
              pages: Vec<PageId>, fields: Vec<FieldChange> }
FieldChange { name: String, old: Option<String>, new: Option<String> }
Group       = Page | Device | Terminal | Cable | Wire | Text | Part | Project | Info
```

- `compare(project, snapshot_of, from, to)` loads the two projects (the
  session's snapshot reader for revisions, the live project for Now), runs
  the existing `diff` and `changed_sheets`, then builds `entries`. It reads
  only; it never touches the document, the undo history or autosave.
- `label` is what a designer calls the thing: the device tag with its type
  (`=10D41+K1B1G1-K1 contactor`), the terminal `X1:3`, the cable tag, the
  wire's number or its two ends, the page name, the text's first words.
  Removed objects are labelled from the older state.
- `FieldChange` values are display strings built from the JSON fields that
  `diff` already finds, with units (mm, V, A) and part numbers by name, and
  nested properties flattened one level (`properties.voltage 24 V → 230 V`).
  Bookkeeping fields stay hidden (`frozen`, internal ids, order keys).
- A moved symbol reads "moved 40 mm right" with the old and new position, not
  two coordinate dumps. A moved page-wide batch (same offset on many
  objects) is folded into one line "12 objects moved together".
- `Entry` order: group, then page order, then label. Stable, so lists and
  exports of the same pair are identical.
- The comparison is cached by (from, to, project revision counter), like the
  current change set, and built once per change, not per frame.

## App

- **Changes card** gets two pickers, "From [B ▾]  To [Now ▾]". Default and
  the one-click case do not change.
- **Compare window** (Ctrl+K "compare revisions", and the Changes card's
  "List"): the entries by group, each row `label · what changed · page`, a
  filter box and group toggles, counts in the header. A row jumps to the page
  and selects the object (removed objects jump to the page and flash the
  ghost). The note and author of both revisions show on top.
- **Canvas**: marks and ghosts as today when `to` is Now. When `to` is a
  revision, the canvas shows that revision read-only from its snapshot, with
  the same marks (added in place, removed as ghosts from `from`). The view
  is clearly labelled "Revision C, read only" and one key returns to Now.
  Editing is only ever on Now.
- **Revision table**: #343 stays the owner of the frame table; this thread
  checks it prints `Project::revisions` (index, date, author, description)
  and refreshes after an issue, an undo of an issue, and a revision opened
  read-only (the table shows that revision's list).

## Output

- `kopercad-cli revisions list <project>` and
  `kopercad-cli revisions diff <project> <from> <to> [--json]` (`now` is a
  valid side). Same `compare`, so scripts and the app agree. The Python
  layer exposes the same call. Exit code is 0 for "differences found" as
  well as "none"; scripting reads the output.
- A "Change list" generated report between two states (table with the same
  groups) goes to `novo-reports`. It is optional and later; the entries are
  built so the report is a thin step.

## Undo, autosave, storage

- Compare is read-only: no command, no undo step, no dirty flag.
- Issuing is unchanged (one command, one undo step, snapshot written in the
  same call). Autosave and crash recovery copy snapshots with the project
  (`copy_snapshots_to`); a test pins that a recovered file compares exactly
  like the original.
- No schema change and no migration. Snapshots stay zlib compressed. If
  size becomes a problem on large projects, storing later snapshots as
  deltas is a separate decision, not part of this.

## Build order (each is its own small PR)

1. This note.
2. `novo-revisions`: `Side`, `compare`, `Entry` and `FieldChange`
   (labels, display values, move folding), with tests on projects built
   through commands and on a recovered file.
3. CLI `revisions list|diff` and the Python call.
4. App: two pickers on the Changes card, and the Compare window.
5. App: read-only revision view on the canvas, revision table check.
6. Issue window note field; optional "Change list" report.

All PRs are drafts until the reviewer marks them READY. No builds are
dispatched from this thread.

## Open questions

1. Is the read-only revision view (step 5) wanted in the first release of
   this, or is the list plus marks against Now enough? Default: build it,
   it is the visual diff the owner asked for.
2. Should unchanged-but-reissued revisions ("no changes since B") be
   hidden in the pickers? Default: shown, they are real events.
3. Generated sheets (BOM, wire list) are compared by content hash only, so
   the list says "sheet changed", not which rows. Row level diff of reports
   is out of scope here. Default: leave it.
