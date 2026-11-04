# Rittal enclosures and mounting plates

## Getting the model (hand-off, no scraping)

Parts window, select a Rittal part, **Import**. KoperCAD opens Rittal's CAD
portal (CADENAS PARTcommunity, `rittal.partcommunity.com`) for the article.
You sign in with your own account and download **STEP** (DXF optional). The
Downloads watcher sees the ZIP and imports it. KoperCAD never signs in,
stores a password or scripts the portal: the CADENAS terms forbid automated
access. The link form is not confirmed with a real session, so the page is
shown as unverified; searching the number on the portal works.

## What is read

- Mounting plate (thin flat body): width, height, thickness, the round holes
  and rectangular openings it is delivered with (plate mm, top left origin,
  x right, y down, seen from the front). Slots and shaped openings are
  counted and listed as "mark from the maker's drawing".
- Enclosure (a box whose description says enclosure): outer size. Width,
  height and depth come from the description (`600 x 760 x 210`) when it has
  them; otherwise ordered by size, which the summary flags as a guess.

The axes are the model's. A maker may model a plate the other way round, so
check one hole on the real plate before drilling. The values are tagged
`drill_basis = maker`.

## Where you find it

- Parts window: Import on a Rittal part (the summary lists what was read).
- Cabinet, Plate setup, Inspector, **Maker plate**: pick the plate part and
  **Use**. The plate gets that size and the drilling plan (Cabinet, Output,
  Export, Drill plan) draws the factory holes as light rings and marks a
  planned hole that lands on one "on a factory hole".

## Imported models are personal

CADENAS terms allow use for your own design work only: no passing on, no
collections, no AI training. KoperCAD therefore:

- keeps the files in your own parts library and catalog, never in the
  repository, test fixtures or example projects;
- leaves them out of **share** bundles even when you confirm the files are
  shareable (a backup for yourself keeps them);
- never uses them to train or tune anything.

Tests use hand-made geometry only. Do not add Rittal files to the repository.
