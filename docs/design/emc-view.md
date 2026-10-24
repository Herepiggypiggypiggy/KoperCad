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

## Net classes on the schematic

**View > EMC net classes** (or Ctrl+K "EMC net classes") colours every wire of the open page by its cable class, through the canvas wire looks (`novo-app/src/app/emc_schematic.rs`). Each wire also carries a small tag with the class word, so the class never rests on colour alone; a guessed class reads "control ?". The class is the one the EMC check uses (`novo_emc::project::input_of`): set by hand (`emc.class`), read from the drawing, or guessed, and the panel says how many are guessed.

The panel counts the classes (a click keeps one class in colour and steps the rest back). Picking a wire shows its class, where the class came from and why, and buttons to set the class by hand or go back to automatic: one undo step each, through the same fix the EMC check applies (`Fix::DeclareClass`). The live view colours wires by potential and wins while it is on; the panel says so.

## Cable card and fixes

Picking a wire that belongs to a cable adds the cable's card to the panel: tag, cores, wires drawn, twisted or not stated, the shield and how each end is made (all round, pigtail, not connected, not stated), and the EMC check's findings about that cable with their rule id and level. A finding the program can act on has an **Apply** button (add a shield core, set both ends all round): one undo step, through the same fix as Check > EMC check (`Fix::AddShield`, `Fix::Terminate360`); the others say "By hand". "No finding about this cable" is not a statement that it is fine. The EMC example now carries an unshielded four-core motor cable (-W2) so the card has something to show.

## Bonding and shield ends on the plate

With the Bonding chip on, each device's bond to the plate is drawn as a dashed outline: a bonded device in the earth colours, a weak or missing bond with a glowing spot and a hover that says why (the Plate bonding hot spot names it). With the Shields chip on, each shield end is a ring on the device: all round in the good colour, pigtail in a risk colour with a tail, not connected in a risk colour with a cross. The legend says what the rings mean (plain ring all round, tail pigtail, cross not connected) and hovering a ring names the cable end. The picture is tested on `qa/fixtures/emc-cabinet-shields.novo` (a galvanised plate, so the drive's bond is a good one, and two shielded cables carrying all three end states; QA 165 and 166). The legend starts rolled up when the canvas is shorter than 700 points, so it does not cover the devices it explains. A weak bond is a rule on the stated plate finish and bonding, not a measurement. Where to find it: View > EMC on the Cabinet plate (Ctrl+K "EMC view on the plate").

## Not in this change

Fixes that move a wire or a device (by hand today), a cable card in the 3D and plate views.
