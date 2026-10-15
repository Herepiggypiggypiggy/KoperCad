# Enclosure sizing helper: handoff

Branch `claude/project-thread-4m13wx`. #402 merged: engine `novo-layout/src/sizing.rs`, bridge `sizing_model.rs`, tests in `crates/novo-layout/tests/`. No `novo-core` change, no build run.

API: `EnclosureCatalog` (user list; `from_csv` accepts `;`, `,` or tab and decimal comma, `to_csv`), `recommend(devices, catalog, request)`, `thermal(enclosure, loss, request)`, `effective_surface`, `sizing_model::sizing_devices(project, structure)`. Part keys read: `width_mm`, `height_mm`, `depth_mm`, `power_loss_w`, `clearance_*_mm`, `load_factor`; device property overrides part.

## How to reach it
- Cabinet view (Ctrl+2): header button "Size enclosure", or palette "Size the enclosure…" (`cabinet.size-enclosure`), opens a floating window (the right side belongs to the inspector, #443): free space, ducts, temperatures, how it stands; result with enclosure, plate fill, inside temperature, "Natural cooling is enough" or "Needs about N W of cooling, or a fan of about V m3/h", notes. Code: `crates/novo-app/src/app/sizing.rs`.
- "Enclosures..." button in the window, or palette `cabinet.enclosures`, opens the list window (edit rows, Add, Remove, Import CSV, Export CSV). Saved on every change as `enclosures.csv` in `%APPDATA%\KoperCAD` (no admin rights).
- Not yet: the Issues-list problem "needs about N W of cooling" (left to the Heat model thread by the lead; needs #409 inside size plus a wall thickness input).

## Traps
- Constants (K_*, surface factors) are planning values, unverified against a maker handbook; the UI must say so.
- The area rule is an estimate, not the `place all` planner.
- Heat model thread (#406) owns per-device heat contribution and the hot-spot map in `thermal.rs`; do not duplicate the balance.
- `depth_reserve` is a flat number until `novo_layout::enclosure` (#409) interior fields are used.
