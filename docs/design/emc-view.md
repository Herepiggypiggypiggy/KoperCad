# EMC view (visualisation lane)

Where the user finds it: **View > 3D > EMC view**, the **EMC** button in the 3D toolbar, and Ctrl+K "EMC".
**Ctrl+K "Open EMC example"** opens a project with a drive, a motor cable, a PLC and an analog input laid out the way a first draft often is.

On the Cabinet: the **EMC** button in the Cabinet toolbar (under More in a narrow window) or Ctrl+K "EMC view on the plate" draws the same picture flat over the mounting plate.

## What it shows

- Cable classes (mains, motor and drive output, 24 V control, analog and sensor, bus, safety, earth) as wire colour, with an earth stripe. Never colour alone: the legend names each class and counts its wires.
- Keep-clear bands round noisy runs (three nested soft strips), a glow round noisy parts, numbered hot spots (H1, H2, ...) with a card.
- An exposure map on the plate in the heat view's ember ramp. It is geometry only (a falloff from noisy parts and runs); the legend says so.
- Loop sheets, bonding graph and shield ends, each its own layer, shown only when the data has any.
- A card per hot spot: level, kind and rule id, the numbers behind it, the assumptions marked default or set by the user, what is involved, and fixes (advice only until the engine can apply commands).

## Honest labelling

"Indicative model, not a field solver. It never shows compliance." is always on the panel. Levels follow `docs/EMC.md`: Rule, Index, Estimate, Field; "not computed" appears only in the not-checked list. Words: "no finding", "design aid, not a test result"; never pass, compliant or certified.

## Data interface

The view reads one plain-data `Picture` (`novo-view3d::emc::picture`): runs with class and reason, nodes with role, hot spots, loops, bonds, shields, an optional field, provenance and assumptions. The view does no physics.

Provenance says where the picture came from: `Synthetic` (demonstration), `Stub` (today), `Engine` (novo-emc). The stub (`novo-app/src/app/emc_stub.rs`) guesses classes from the symbol family, pin names and wire colours, measures parallel stretches and distances from the routed wires, and says so on the panel. When `novo-emc` exists, only an adapter that builds a `Picture` from it replaces the stub; `emc_view` and the overlays stay.

## Not in this change

The schematic net-class overlay, one-click fixes that edit the project, per-cable cards.
