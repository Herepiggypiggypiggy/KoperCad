# Explanation text inventory (baseline for the remaining areas)

Measured 2026-11-03 on main 5108a1e plus the first help-icon PR, at 1366x768, from the running app: every label of more than 8 words in the widget tree after each screenshot of 112 scenarios (a sample of every area, about one in six). Source: `KOPERCAD_QA_TEXT_AUDIT=<file> scripts/qa.sh ...` writes scenario, size, shot, words, text.

Static count for the rest (source literals of 8 or more words, hover texts included, tests excluded): 1,244 literals, 16,120 words, in 183 files of `crates/novo-app/src/app/` (biggest: bench.rs 773, cabinet.rs 742, preferences.rs 378, heat_flow.rs 345, cabinet_routes.rs 336, model_labels.rs 329, view3d_runs.rs 319, workshop.rs 314, emc_view.rs 299, live.rs 296).

Runtime sample (distinct labels, words) by scenario area, after the 3D Heat/Hoist/Drive run changes. Goal: close to zero outside the "?".

| Area | Distinct long labels | Words |
|---|---|---|
| Learning | 57 | 1251 |
| Parts | 84 | 1199 |
| Macros | 53 | 1076 |
| Learn | 41 | 895 |
| Cabinet | 42 | 827 |
| Reports | 27 | 615 |
| Assistant | 29 | 591 |
| Fixtures | 28 | 543 |
| Schematic | 28 | 500 |
| 3D | 22 | 390 |
| Check | 17 | 254 |
| Simulate | 19 | 228 |
| Add-ons | 16 | 221 |
| Terminals | 14 | 197 |
| Live | 14 | 180 |
| Heat | 10 | 154 |
| Hostile | 9 | 144 |
| Examples | 10 | 118 |
| Layout | 9 | 115 |
| Windows | 10 | 102 |
| Options | 6 | 96 |
| Journeys | 7 | 95 |
| Start | 8 | 87 |
| Editor | 4 | 84 |
| Commissioning | 7 | 82 |
| Workspaces | 5 | 60 |
| Cables | 4 | 46 |
| Regressions | 1 | 42 |
| Help | 3 | 39 |
| Command search | 2 | 24 |
| Revisions | 2 | 24 |
| Checks | 2 | 21 |
| Files | 1 | 11 |
| Appearance | 1 | 10 |
| Command field | 1 | 10 |
| Palette | 1 | 9 |
| **Total** | **594** | **10340** |

Not in the sample: hover tooltips (hidden until pointed at, not counted here), the painted canvas, and text shorter than 9 words. The status bar hints ("Q or Space to place · Ctrl+K to search", the X/Y readout with "Home zoom to fit") appear in 40 scenarios.

## Every long label (area, words, scenario, text)

### 3D

- 40 words, `36-run-card-checks-fixture`: Bend radius (model): at x 32, y 122 mm the ducts leave about 43 mm, the wire 8.3 mm needs 50 mm (6 x its diameter, a planning value): widen the ducts or lay it round the corner by hand
- 40 words, `36-run-card-checks-fixture`: Bend radius (model): at x 32, y 188 mm the ducts leave about 43 mm, the wire 8.3 mm needs 50 mm (6 x its diameter, a planning value): widen the ducts or lay it round the corner by hand
- 34 words, `111-3d-heat-vents-hover-1920`: Through-flow 81 m³/h (air in 81, out 81) for 14 W of device loss: the air warms about 1 K passing through, adequate cooling; the air carries out about 4 W (30 %) (model)
- 24 words, `36-run-card-checks-fixture`: Not checked: how stiff this cable is, its temperature rating, the real surface temperature of a device, corners at terminals and in free air
- 19 words, `36-run-card-checks-fixture`: Heat (model): 80 mm or more from every other device with a known loss of 5 W or more
- 17 words, `196-workpack-wiring-opening-cards`: Exported /tmp/kopercad-test-user-31925/home/Documents/KoperCAD/qa 196 work cards.pdf. Print at 100 % (actual size), then cut each sheet in half
- 17 words, `196-workpack-wiring-opening-cards`: Exported …/KoperCAD/qa 196 work cards.pdf. Print at 100 % (actual size), then cut each sheet in half
- 17 words, `500-workpack-cut-strips`: Exported /tmp/kopercad-test-user-31925/home/Documents/KoperCAD/qa 500 work cards.pdf. Print at 100 % (actual size), then cut each sheet in half
- 17 words, `500-workpack-cut-strips`: Exported …/KoperCAD/qa 500 work cards.pdf. Print at 100 % (actual size), then cut each sheet in half
- 16 words, `196-workpack-wiring-opening-cards`: The enclosure and the mounting plate (the plate is blue). Next puts on the first part.
- 16 words, `36-run-card-checks-fixture`: Fill is a planning value: the wire cross-sections added up against the share the rules allow
- 16 words, `500-workpack-cut-strips`: Cut a DIN rail to 385 mm and fit it level at x 55, y 238.75
- 14 words, `36-run-card-checks-fixture`: Duct at y 122 mm: 26 % full of 50 % allowed, 8 wires
- 14 words, `36-run-card-checks-fixture`: Duct at x 32 mm: 36 % full of 50 % allowed, 11 wires
- 14 words, `36-run-card-checks-fixture`: Duct at y 188 mm: 14 % full of 50 % allowed, 4 wires
- 13 words, `111-3d-heat-vents-hover-1920`:  46 °C, 21 K under its limit. 3 devices have no limit data.
- 13 words, `111-3d-heat-vents-hover-1920`:  49 °C, 19 K under its limit. 3 devices have no limit data.
- 12 words, `111-3d-heat-vents-hover-1920`: Air in 81, out 81 m³/h (model); 0 m³/h (0 %) apart
- 10 words, `36-run-card-checks-fixture`: Measured along the ducts from terminal to terminal, loops included.
- 9 words, `111-3d-heat-vents-hover-1920`: Heat: Coarse 3D · it 85 · 10 s
- 9 words, `111-3d-heat-vents-hover-1920`: Heat: Coarse 3D · it 200 · 15 s
- 9 words, `196-workpack-wiring-opening-cards`: The part this step adds is shown in blue.

### Add-ons

- 24 words, `561-addons-engine`: Demo engine stopped: it hit an error and stopped (wasm `unreachable` instruction executed). Turn it off and on in Add-ons to start it again.
- 23 words, `178010-addon-project-data`: An add-on is a file ending in .kcaddon. Install one to add features; nothing is installed for you and nothing is sent anywhere.
- 21 words, `561-addons-engine`: It stopped: it hit an error and stopped (wasm `unreachable` instruction executed). Turn it off and on again to start it.
- 16 words, `561-addons-engine`: Nobody you trust signed this file. Install it only if you know where it came from.
- 15 words, `561-addons-engine`: A demonstration add-on with a small sandboxed engine: Start and Stop buttons drive its lamps.
- 13 words, `178010-addon-project-data`: Removed 1 block(s) of controllers data from this project. Undo brings it back.
- 13 words, `561-addons-engine`: Its busiest call used 0% of its step budget (2 calls so far).
- 13 words, `561-addons-engine`: Its busiest call used 0% of its step budget (4 calls so far).
- 12 words, `561-addons-engine`:  Unsigned, from file (you accepted it): the package has no signature
- 11 words, `178010-addon-project-data`: extra features you install from a file, without leaving the program
- 11 words, `178010-addon-project-data`: 1 block, 5 KB, add-on not installed: kept as it is
- 10 words, `178010-addon-project-data`: Drop a .kcaddon file on this window to install it.
- 10 words, `561-addons-engine`: •  Add entries to the menus and the command search
- 10 words, `561-addons-engine`: The buttons ask the engine; it answers with lamp states.
- 10 words, `561-addons-engine`: X 408.0 mm  Y 52.0 mm   Home zoom to fit
- 9 words, `561-addons-engine`:  Unsigned, from file: the package has no signature

### Appearance

- 10 words, `340-high-contrast`: X 416.0 mm  Y 72.0 mm   Home zoom to fit

### Assistant

- 85 words, `09-ask-claude`: Your messages, and the project data allowed above, are sent over the internet to Anthropic's Claude API using your own API key. Anthropic's terms and your account's data settings apply to them. Claude can be wrong: check
- 45 words, `09-ask-claude`: Only the messages you type are sent, a picture of the window or a datasheet PDF if you choose to send one, and the text of a button that says “with Claude” when you press it. Claude cannot look at your drawing on its own
- 43 words, `09-ask-claude`: The key is saved as a file in your user profile (not encrypted, not in the project file; only its last four characters are ever shown). Anyone who can read your profile can read it. Remove it here when you stop using Cla
- 38 words, `11b-ask-claude-build-starter-1920`: Place macro Motor starter DOL on page 1: devices -F1, -F2, -F3, -K1, -M1, -S1, -S2. Pins left open for you to connect: -F1:1, -F1:3, -F1:5, protective earth terminal pin 1 (symbol 14), -F3:1, -K1:A2. Ctrl+Z undoes it.
- 36 words, `09-ask-claude`: Everything in the open project can be sent when Claude asks: devices, parts, wires, terminals, cabinet layout, notes, the design-check problems and wire-number conflicts. Use this only for projects you are allowed to sha
- 35 words, `11b-ask-claude-build-starter-1920`: Place macro Motor starter DOL on page 1: devices -F1, -F2, -F3, -K1, -M1, -S1, -S2. Pins left open for you to connect: -F1:1, -F1:3, -F1:5, protective earth terminal pin 1 (symbol 14), -F3:1, -K1:A2
- 25 words, `09-ask-claude`: The project name, page names and titles, device tags and counts are sent when Claude asks for them. No part numbers, notes, wires or positions.
- 24 words, `09-ask-claude`: Optional. Uses your own Anthropic API key and your own API credits. KoperCAD has no account with Anthropic and sees none of your usage.
- 23 words, `11b-ask-claude-build-starter-1920`: The starter is in the draft with its checks run. Review the change set and press Apply to put it on page 1.
- 20 words, `11b-ask-claude-build-starter-1920`: The checks find drawing problems. They do not show that the design is right or that it meets a standard.
- 19 words, `09-ask-claude`: Most careful; best for design review and multi-step edits. List price $4.00 in / $20.00 out per million tokens.
- 16 words, `09-ask-claude`: The outline, plus the full details of what you have selected at the moment Claude asks.
- 15 words, `09-ask-claude`: Buttons that say “with Claude” send the text shown on screen, whatever you chose here.
- 14 words, `09-ask-claude`: Costs are estimates from Anthropic's list prices. Your Anthropic account shows the real bill.
- 13 words, `11b-ask-claude-build-starter-1920`: 20 wires given or changed a number (wires of one net share it)
- 12 words, `11b-ask-claude-build-starter-1920`: I will plan this, build it in a draft, and check it.
- 11 words, `09-ask-claude`: Scroll down and read “4. Agree” to be able to agree.
- 10 words, `09-ask-claude`: X 72.0 mm  Y 1496.0 mm   Home zoom to fit
- 10 words, `09-ask-claude`: X 72.0 mm  Y 100.0 mm   Home zoom to fit
- 10 words, `09-ask-claude`: X 388.0 mm  Y 236.0 mm   Home zoom to fit
- 10 words, `09-ask-claude`: X 524.0 mm  Y 276.0 mm   Home zoom to fit
- 10 words, `09-ask-claude`: X 24.0 mm  Y 2028.0 mm   Home zoom to fit
- 10 words, `09-ask-claude`: X 24.0 mm  Y 136.0 mm   Home zoom to fit
- 10 words, `11b-ask-claude-build-starter-1920`: Test replay: answers come from a file, nothing is sent
- 10 words, `13b-ask-claude-light-1920`: X 388.0 mm  Y 24.0 mm   Home zoom to fit
- 10 words, `13b-ask-claude-light-1920`: X -8.0 mm  Y -156.0 mm   Home zoom to fit
- 9 words, `09-ask-claude`: No key yet. Create one at console.anthropic.com, API keys.
- 9 words, `11b-ask-claude-build-starter-1920`: New devices: -F1, -F2, -F3, -K1, -M1, -S1, -S2
- 9 words, `11b-ask-claude-build-starter-1920`: New warning: terminal PE (no tag): 1 not connected

### Cabinet

- 52 words, `471-emc-fixit-move-out-of-field-light`: -A1:AI1 to -B1:1 (Analog and sensor) picks up about 0.704 mV at 50 Hz and about 975 mV on a 10 A/µs current step from the wires beside it; planning value 10.0 mV for this class. The return is taken as the plate. A model 
- 45 words, `471-emc-fixit-move-out-of-field-light`: Rules not built yet, so they say nothing either way: EMC-03 crossing angles, EMC-06 shield carried through terminals, EMC-08 motor cable length against the filter, EMC-10 return through the plate, EMC-13 coil suppression
- 42 words, `471-emc-fixit-move-out-of-field-light`: Whether a filter is wired in series with its drive: the filter rule counts a filter as wired when two of its pins carry a wire, so a filter wired in parallel, or with both wires on one side, would count too.
- 39 words, `471-emc-fixit-move-out-of-field-light`: -A1 sits in a field of about 208 µT from the currents drawn (mostly -T1:V to -M1:V1 at 50 Hz); the planning value is 12.6 µT. A model of the stated currents on the routed wires, not a measurement.
- 36 words, `471-emc-fixit-move-out-of-field-light`: The magnetic field estimate covers the differential-mode field of the currents it lists: no common-mode current, no capacitive coupling, no eddy currents in a steel plate at mains frequency, no riser wires between plate 
- 35 words, `1612-rails-list-refuses-duct-overlap-1920`: Edit a value to move or resize, or drag the selected one on the plate: its body to move, either end handle to cut it shorter or longer. Delete removes it. Ctrl+Z undoes each step.
- 33 words, `471-emc-fixit-move-out-of-field-light`: ESD current paths through the door, the strap and the frame: the door strap, the devices on the door and the wires across the hinge are checked, the discharge itself is not modelled.
- 32 words, `471-emc-fixit-move-out-of-field-light`: Devices on the door in the plate field estimate (EMC-19): they stand in another plane than the plate, so the field model does not cover them and they get no EMC-19 result.
- 28 words, `471-emc-fixit-move-out-of-field-light`: Design aid, not a test result. Findings are rules on what is drawn, geometric indices and closed-form estimates with planning distances from common practice; clause numbers are unverified.
- 28 words, `471-emc-fixit-move-out-of-field-light`: -A1:AI1 to -B1:1 (Analog and sensor) runs in the same duct as -T1:U to -M1:U1 (Motor (drive output)) for 530 mm: these classes need 300 mm between them.
- 28 words, `471-emc-fixit-move-out-of-field-light`: -A1:AI1 to -B1:1 (Analog and sensor) runs in the same duct as -T1:V to -M1:V1 (Motor (drive output)) for 530 mm: these classes need 300 mm between them.
- 28 words, `471-emc-fixit-move-out-of-field-light`: -A1:AI1 to -B1:1 (Analog and sensor) runs in the same duct as -T1:W to -M1:W1 (Motor (drive output)) for 530 mm: these classes need 300 mm between them.
- 27 words, `1612-rails-list-refuses-duct-overlap-1920`: A duct cannot lie over 4 other ducts: move it clear, or end it against the side of the other duct for a T or a corner
- 19 words, `471-emc-fixit-move-out-of-field-light`: The motor wires of -T1 are single wires, not a shielded cable: put them in a shielded motor cable.
- 18 words, `18132-cabinet-shift-click-then-drag`: • F1 belongs in the top zone, which is above S0 (rule `Q F G zone=top rail=shared class=power`).
- 18 words, `18132-cabinet-shift-click-then-drag`: • F2 belongs in the top zone, which is above S0 (rule `Q F G zone=top rail=shared class=power`).
- 18 words, `471-emc-fixit-move-out-of-field-light`:     Induced voltage (estimate): about 2.4 V (wire height and 10 A/us assumed; order of magnitude only, not measured)
- 17 words, `471-emc-fixit-move-out-of-field-light`: -A1 sits 10 mm from -T1: keep sensitive devices at least 100 mm away from a drive.
- 17 words, `471-emc-fixit-move-out-of-field-light`: -T1 has no mains filter. If its part has one built in, say so (emc.filter = built-in).
- 15 words, `100-cabinet-emc-left-out`: 4 wires with no duct route are left out of this view (see Route all)
- 15 words, `18132-cabinet-shift-click-then-drag`: Equal space between the devices from left to right (needs three). Needs 3 selected devices.
- 15 words, `471-emc-fixit-move-out-of-field-light`: Off: the findings below are a preview and do not appear in the Issues list.
- 14 words, `1612-rails-list-refuses-duct-overlap-1920`: 2 pages are in no group or cabinet. Their devices say where they belong.
- 13 words, `471-emc-fixit-move-out-of-field-light`: By hand: Keep the two runs 300 mm apart, or in separate ducts.
- 13 words, `471-emc-fixit-move-out-of-field-light`: By hand: Keep the two runs 1000 mm apart, or in separate ducts.
- 13 words, `471-emc-fixit-move-out-of-field-light`: Compliance with any standard. Compliance is shown by measurement on the finished machine.
- 12 words, `18132-cabinet-shift-click-then-drag`: • Lies under a duct (25 x 54 mm): move it clear
- 12 words, `18132-cabinet-shift-click-then-drag`: • Lies under a duct (25 x 45 mm): move it clear
- 12 words, `471-emc-fixit-move-out-of-field-light`: By hand: Run go and return together, or in one twisted cable.
- 12 words, `471-emc-fixit-move-out-of-field-light`: Radiated and conducted emission margins (needs a measurement or a full-wave solver).
- 12 words, `471-emc-fixit-move-out-of-field-light`: Anything above about 30 MHz: cable lengths approach a wavelength fraction there.
- 11 words, `100-cabinet-emc-left-out`: These devices need a larger plate than 450 x 550 mm.
- 11 words, `471-emc-fixit-move-out-of-field-light`:     Field at the device (largest over its footprint) (estimate): 208 µT
- 11 words, `471-emc-fixit-move-out-of-field-light`: Real filter insertion loss and drive-maker limits on motor cable length.
- 10 words, `100-cabinet-emc-left-out`: Indicative model, not a field solver. It never shows compliance.
- 10 words, `471-emc-fixit-move-out-of-field-light`: X 600.0 mm  Y 12.0 mm   Home zoom to fit
- 10 words, `471-emc-fixit-move-out-of-field-light`: By hand: Move the device to keep 660 mm clear.
- 10 words, `471-emc-fixit-move-out-of-field-light`: X 624.0 mm  Y -32.0 mm   Home zoom to fit
- 9 words, `100-cabinet-emc-left-out`: Soft band round a run: keep sensitive wires out
- 9 words, `166-emc-shields-plate-1920`: Ring with a tail: pigtail, the longer, the worse
- 9 words, `471-emc-fixit-move-out-of-field-light`:     Induced at the line frequency (RMS) (estimate): 0.704 mV
- 9 words, `471-emc-fixit-move-out-of-field-light`:     Induced on the current step (peak) (estimate): 975 mV

### Cables

- 14 words, `66-cables-proposals-scroll`: No cables yet. Wires between two locations (+CAB to +FIELD) are proposed as cables.
- 12 words, `66-cables-proposals-scroll`: No cables yet. Accept the proposed ones, or draw wires between locations.
- 10 words, `66-cables-proposals-scroll`: X -12.0 mm  Y 148.0 mm   Home zoom to fit
- 10 words, `66-cables-proposals-scroll`: X 16.0 mm  Y 1696.0 mm   Home zoom to fit

### Check

- 39 words, `1710-connection-points`: 2 connections between pins + 0 supply and PE leads = 2 wires to cut; 4 wire labels (one per wire end); 2 wire pieces drawn (a connection can be drawn in several pieces, and supply feeds count too).
- 28 words, `11-emc-check`: -A1:AI1 to -B1:1 (Analog and sensor) runs in the same duct as -T1:U to -M1:U1 (Motor (drive output)) for 540 mm: these classes need 300 mm between them.
- 18 words, `11-emc-check`:     Induced voltage (estimate): about 2.5 V (wire height and 10 A/us assumed; order of magnitude only, not measured)
- 17 words, `11-emc-check`: -A1 sits 0 mm from -T1: keep sensitive devices at least 100 mm away from a drive.
- 16 words, `152-emc-field-check-window`: +0°  rated current of -T1 (planning value: the smaller rated current); phase from the pin names
- 16 words, `152-emc-field-check-window`: -120°  rated current of -T1 (planning value: the smaller rated current); phase from the pin names
- 16 words, `152-emc-field-check-window`: +120°  rated current of -T1 (planning value: the smaller rated current); phase from the pin names
- 14 words, `152-emc-field-check-window`: -A1:AI1 to -B1:1: 0.704 mV at 50 Hz, 975.3 mV on a current step
- 12 words, `1710-connection-points`: 2 CONNECTIONS BETWEEN DEVICE PINS (SUPPLY AND PE LEADS: SEE WIRE PRODUCTION)
- 10 words, `11-emc-check`: X 324.0 mm  Y 144.0 mm   Home zoom to fit
- 10 words, `11-emc-check`: X 164.0 mm  Y 148.0 mm   Home zoom to fit
- 10 words, `152-emc-field-check-window`: X 60.0 mm  Y -28.0 mm   Home zoom to fit
- 10 words, `152-emc-field-check-window`: X 148.0 mm  Y 72.0 mm   Home zoom to fit
- 10 words, `1710-connection-points`: X 104.0 mm  Y 68.0 mm   Home zoom to fit
- 10 words, `1710-connection-points`: X 104.0 mm  Y 92.0 mm   Home zoom to fit
- 9 words, `152-emc-field-check-window`: planning value 12.6 µT (largest over the device's footprint)
- 9 words, `152-emc-field-check-window`: planning value 10.0 mV; return taken as the plate

### Checks

- 11 words, `1600-cooling-tower-shorts`: Opened /tmp/kopercad-qa-open-31925/cooling-tower-shorts.novo. Older file format: Save As keeps a .kopercad copy
- 10 words, `1600-cooling-tower-shorts`: X 96.0 mm  Y 80.0 mm   Home zoom to fit

### Command field

- 10 words, `140-plc-macro-search-entry`: X -24.0 mm  Y 92.0 mm   Home zoom to fit

### Command search

- 13 words, `631-search-wire-numbers-runs-a-command`: Numbers on wires and potentials: give them, renumber, check, fix one by hand.
- 11 words, `631-search-wire-numbers-runs-a-command`: Choose an action on the left to see what it numbers.

### Commissioning

- 17 words, `225-wire-test-list-whole-rows`: The bench cut list and Wire-it count connections (25), not drawn wires, so the numbers can differ.
- 15 words, `225-wire-test-list-whole-rows`: Design aid: expected values are planning values from the drawing, not a compliance test record.
- 13 words, `225-wire-test-list-whole-rows`: Zero the test leads first. For OL or over range press Open circuit.
- 10 words, `420-wire-test-sheet`: MISS: a punch item is open on the Punch tab
- 9 words, `225-wire-test-list-whole-rows`: 27 wires in the drawing: 0 tested, 0 missed
- 9 words, `225-wire-test-list-whole-rows`: Disconnect electronics and surge arresters before an insulation test
- 9 words, `420-wire-test-sheet`: 27 wires in the drawing: 1 tested, 1 missed

### Editor

- 45 words, `92-enter-in-dialog`: By the end the start button sits on a PLC input and the lamp on a PLC output, and you have read their addresses in the I/O list. The I/O list and the PLC tag export follow the drawing, so nobody types an address twice.
- 16 words, `95-new-project-closes-build-mode`: The design has 0 errors and 1 warning in Issues. They can change what you build.
- 13 words, `95-new-project-closes-build-mode`: Check the plate 450 x 550 mm and the plan before you start
- 10 words, `92-enter-in-dialog`: X 104.0 mm  Y 56.0 mm   Home zoom to fit

### Examples

- 23 words, `2442-gallery-filling`: Live potentials: the plant is at rest. This page has no switched contacts; use Why? to ask about a wire or a device
- 14 words, `761-gallery-mcc-menu`: A new project with the pages, tags and structure of a kind of machine.
- 13 words, `761-gallery-mcc-menu`: Choose a template or example on the left to see what it is.
- 11 words, `2440-gallery-door`: Any template or example also runs from Ctrl+K by its name.
- 10 words, `2440-gallery-door`: X 4.0 mm  Y 64.0 mm   Home zoom to fit
- 10 words, `2442-gallery-filling`: Large: filling line with PLC. Illustration, not a safety design.
- 10 words, `761-gallery-mcc-menu`: X -28.0 mm  Y 124.0 mm   Home zoom to fit
- 9 words, `2440-gallery-door`: Finished projects to open, look through and learn from.
- 9 words, `2440-gallery-door`: Small: roller door, reversing. Illustration, not a safety design.
- 9 words, `2442-gallery-filling`: Open example: bottle filling line (large, PLC, drive, safety)

### Files

- 11 words, `22-project-extension`: Opened /home/claude/kopercad/crates/novo-tests/tests/golden/v0.1-motor-starter.novo. Older file format: Save As keeps a .kopercad copy

### Fixtures

- 68 words, `06-fixture-crane-import`: Drawn: 2 circuit-diagram pages, 4 symbols from KoperCAD's library, 2 interruption points (links between pages), 4 wires. The picture of each symbol is KoperCAD's nearest match (a box where none fits); positions, pin coun
- 38 words, `06-fixture-crane-import`: 1 section and location, 2 pages, 2 devices, 1 terminal strip, 2 terminals, 2 cables, 2 cable cores, 6 symbols drawn, 4 wires, 1 free text. Left out: 1 generated report page, 2 report table cells and texts.
- 29 words, `06-fixture-crane-import`: The configurator's own project check says: GOED MET OPMERKINGEN (10 automatische controles: 9 in orde, 0 met een fout, 1 met een waarschuwing). KoperCAD does not repeat that check.
- 27 words, `06-fixture-crane-import`: Inferred (EPLAN's rules, rebuilt): which object is a device, terminal or cable, and which pages are circuit diagrams or generated lists. Re-check any of these that matter.
- 25 words, `21-plc-io-list`: row 3: Duplicate: E0.0 (S2) has the same address as E0.0 (S1, row 2); the addresses of the DI signals are assigned by the project
- 25 words, `21-plc-io-list`: row 8: Duplicate: A0.0 (Y1) has the same address as A0.0 (K1, row 5); the addresses of the DQ signals are assigned by the project
- 24 words, `06-fixture-crane-import`: Exact (as the file writes it): device tags, part numbers, terminal texts, cable and core numbers, page numbers and titles, project number and title.
- 22 words, `06-fixture-crane-import`: 1 exchange file. - no kopercad-export.xml next to the file; the script version is unknown The export script is up to date.
- 20 words, `06-fixture-crane-import`: 2 report table cells and texts: the cells of the generated report pages; KoperCAD generates its own from the data
- 19 words, `06-fixture-crane-import`: Net check: KoperCAD's own nets of the drawn pages equal the nets EPLAN's rules give, over 10 connection points.
- 18 words, `05-fixture-motor-starter`: No panel yet. Create a cabinet, or give a page a panel name (+K1), to lay it out.
- 18 words, `06-fixture-crane-import`: 1 generated report page: contents, parts and terminal lists EPLAN generated; KoperCAD generates its own from the data
- 18 words, `06-fixture-crane-import`: 4 wires follow the runs and nodes the file implies: the corners are at its nodes, not guessed.
- 18 words, `06-fixture-crane-import`: 2 cable cores were tied to their cable by the reference the file stores (exact), not by tag.
- 16 words, `06-fixture-crane-import`: Device renaming in the configurator: 3 devices checked, 1 renamed, 1 unchanged, 1 exempt, 0 collisions.
- 16 words, `21-plc-io-list`: row 9: Skipped: M9 (Spare): no signal type that can be told (type "XX", address "")
- 14 words, `06-fixture-crane-import`: Crane configurator output "demo", version 2 (the newest of 2 versions; not read: v1).
- 14 words, `21-plc-io-list`: 8 signals on 3 new cards, one page and one terminal strip per card.
- 14 words, `21-plc-io-list`: 6 signals on 3 new cards, one page and one terminal strip per card.
- 13 words, `06-fixture-crane-import`: Order data from the configurator: project DEMO-1, customer Demo Customer, Demo slewing crane.
- 12 words, `06-fixture-crane-import`: An import only adds. Anything that exists keeps what the project says.
- 12 words, `06-fixture-crane-import`: Net check: 1 connection points that face a direction and join nothing.
- 11 words, `05-fixture-motor-starter`: Opened /tmp/kopercad-qa-open-31925/v0.1-motor-starter.novo. Older file format: Save As keeps a .kopercad copy
- 11 words, `21-plc-io-list`: Imported 8 signals: 3 cards with a page each, 8 terminals
- 11 words, `21-plc-io-list`: Imported 6 signals: 3 cards with a page each, 6 terminals
- 10 words, `05-fixture-motor-starter`: X 180.0 mm  Y 144.0 mm   Home zoom to fit
- 10 words, `06-fixture-crane-import`: Imported 21 records from DEMO-1 v2. Undo takes them back
- 10 words, `06-fixture-crane-import`: X 384.0 mm  Y 276.0 mm   Home zoom to fit

### Heat

- 52 words, `18110-status-bar-heat-chip`: 8 wires in ducts are on no run the model knows a current for. Where they are the other phases or the return of a run counted above, their heat is already in the total (the 3 or 2 conductors); control and signal wires, or
- 13 words, `16163-heat-refuses-tiny-enclosure`: Cannot simulate: 22 of 22 devices lie outside the enclosure: check its size
- 13 words, `16163-heat-refuses-tiny-enclosure`: 22 of 22 loss figures are planning estimates; check them against maker data.
- 13 words, `18110-status-bar-heat-chip`:  51 °C, 17 K under its limit. 3 devices have no limit data.
- 13 words, `18110-status-bar-heat-chip`: 7 of 7 loss figures are planning estimates; check them against maker data.
- 12 words, `18110-status-bar-heat-chip`: Shows where the heat goes; temperatures are a first look. Reduced model.
- 11 words, `136-cooler-layout-needs-plate`: Cooler layout needs a plate: set its size, then search again.
- 9 words, `18110-status-bar-heat-chip`: Heat: Coarse 3D · it 95 · 6 s
- 9 words, `18110-status-bar-heat-chip`: Not converged: balance 15.7 % off, drift 0.00 K
- 9 words, `18110-status-bar-heat-chip`: Heat: Coarse 3D · it 110 · 6 s

### Help

- 16 words, `80-manual-help`: The design has 7 errors and 8 warnings in Issues. They can change what you build.
- 13 words, `80-manual-help`: Check the plate 600 x 800 mm and the plan before you start
- 10 words, `80-manual-help`: X 68.0 mm  Y 168.0 mm   Home zoom to fit

### Hostile

- 24 words, `605-hostile-empty-cabinet-simulations`: No cabinet has a mounting plate with something to build yet. Create a cabinet, set its plate and place devices on the Cabinet canvas.
- 22 words, `605-hostile-empty-cabinet-simulations`: Heat: Cannot simulate: the enclosure interior (width, height, depth) is not set. Set it in the Cabinet: Size enclosure or Plate setup
- 21 words, `605-hostile-empty-cabinet-simulations`: Cannot simulate: the enclosure interior (width, height, depth) is not set. Set it in the Cabinet: Size enclosure or Plate setup
- 21 words, `605-hostile-empty-cabinet-simulations`: Cannot search: the enclosure interior (width, height, depth) is not set. Set it in the Cabinet: Size enclosure or Plate setup
- 14 words, `602-hostile-workspace-switch-mid-operation`:  Start a wire on a pin, junction or wire · Esc to stop
- 13 words, `605-hostile-empty-cabinet-simulations`: Cannot search: set the enclosure size and the plate first (Size the enclosure)
- 10 words, `602-hostile-workspace-switch-mid-operation`: X 160.0 mm  Y 148.0 mm   Home zoom to fit
- 10 words, `602-hostile-workspace-switch-mid-operation`: X 112.0 mm  Y 100.0 mm   Home zoom to fit
- 9 words, `605-hostile-empty-cabinet-simulations`: No finding: not a statement the design is fine

### Journeys

- 25 words, `58-acceptance-plc-card`: Added card -A1 (DI16 DQ8). Its channels are drawn when you send a contact, sensor or coil to it: select one and run To PLC
- 15 words, `10-journey-export`: Exported /tmp/kopercad-test-user-31925/home/Documents/KoperCAD/Motor starter (example).pdf, with a Design checks page at the end (1 warning remain)
- 15 words, `10-journey-export`: Exported …/KoperCAD/Motor starter (example).pdf, with a Design checks page at the end (1 warning remain)
- 10 words, `12-journey-new-project`: X 92.0 mm  Y 124.0 mm   Home zoom to fit
- 10 words, `12-journey-new-project`: X 96.0 mm  Y 96.0 mm   Home zoom to fit
- 10 words, `58-acceptance-plc-card`: X 388.0 mm  Y 192.0 mm   Home zoom to fit
- 10 words, `58-acceptance-plc-card`: X 208.0 mm  Y 56.0 mm   Home zoom to fit

### Layout

- 27 words, `10-light-look`: The live view switches states at once and shows an arrow on running motors instead of spinning them. Fans and air in the 3D view stand still.
- 15 words, `7376-plc-rows`: Spreadsheets for bulk edit, a machine's I/O list and the tags of a PLC programmer.
- 12 words, `7376-plc-rows`: Choose an item on the left to see what it brings in.
- 11 words, `7376-plc-rows`: A DXF or DWG drawing behind the page, to trace over.
- 11 words, `7376-plc-rows`: Another tool's project or plant, brought in as pages and devices.
- 10 words, `10-light-look`: X 404.0 mm  Y 40.0 mm   Home zoom to fit
- 10 words, `7376-plc-rows`: X -28.0 mm  Y 112.0 mm   Home zoom to fit
- 10 words, `7376-plc-rows`: X 40.0 mm  Y 72.0 mm   Home zoom to fit
- 9 words, `7376-plc-rows`: Part data for the catalog, from files you have.

### Learn

- 69 words, `211-learn-kitting`: . It writes "<project> kitting list.pdf" next to the project, with the pick lists, the saw plan and the shopping list, and says which numbers come from the drawing and which are planning values. It is also part of the wo
- 60 words, `211-learn-kitting`:  packs the rails and ducts of the cabinet into bars, taking the saw cut off every cut. It names the bars to buy and the offcuts to keep; open it to see each bar: "new 2000 mm", then the cuts, then "mm left, keep" or "(sc
- 58 words, `211-learn-kitting`: The first line counts what to buy: bars, reels and lines on the shopping list. An orange line under it says what makes the list incomplete, for example "devices or cables have no part: they cannot be on the shopping list
- 48 words, `211-learn-kitting`:  folds follow the assembly checklist: Prepare, Mount, Wire, Label. Each line has the quantity, the unit and what it is for, for example "New bar, to buy. For" the rails that come from it. The step numbers match the check
- 48 words, `530-learn-heat-claims`: , the biggest source: how hot a device runs follows its own loss and its size, so one 20 W device stands out from several small ones side by side. On this cabinet the preview ranks the devices the way the solver does, bu
- 41 words, `211-learn-kitting`:  merges the stock lines with the parts list: From, Item, Need, Have, Buy. What you already hold is taken off before anything is bought. Give a stock item an order number and it merges into the parts-list line with that n
- 37 words, `211-learn-kitting`:  works that out from the drawing and the stock you hold. Counts of rails, devices, wires and holes come from the drawing. Bar and reel lengths, the saw cut and the packs are planning values you set.
- 31 words, `120-drawing-standard-click-ticks`: The standard is saved in the project file, so a colleague opens it the same way. On its own it redraws nothing: symbols already on the page stay as they are.
- 30 words, `120-drawing-standard-click-ticks`: Nothing to set up: a new group or cabinet continues the names this project already has. These letters only start a project that has none yet. Yours, for every project.
- 30 words, `211-learn-kitting`: Exported kitting list (6 bars and 0 reels to buy, 3 lines on the shopping list): /tmp/kopercad-test-user-31925/home/Documents/KoperCAD/Lesson_ Kitting_ what to take from the shelf and what to buy kitting list.pdf
- 30 words, `211-learn-kitting`: Exported kitting list (6 bars and 0 reels to buy, 3 lines on the shopping list): …/KoperCAD/Lesson_ Kitting_ what to take from the shelf and what to buy kitting list.pdf
- 26 words, `211-learn-kitting`: : the bar lengths you buy, the saw cut, the offcut length you keep, the reel length, the share held back per reel, and the packs. 
- 25 words, `211-learn-kitting`: A build starts at the shelf: which rails and ducts to saw, which reels and packs to take, what is missing and must be bought. 
- 19 words, `211-learn-kitting`: 22 devices or cables have no part: they cannot be on the shopping list until a part is assigned
- 17 words, `120-drawing-standard-click-ticks`: Printed on the cover pages (Front matter). Leave a field empty and the page shows a dash.
- 17 words, `211-learn-kitting`: New bar, to buy. For +CAB1 R1, +CAB1 R2, +CAB1 R3, +CAB1 R4, +CAB1 R5, +CAB1 R6
- 17 words, `530-learn-heat-claims`:  1.5 W. Together that is 34.5 W of heat that the cabinet has to get rid of.
- 17 words, `530-learn-heat-claims`: . Each chip switches one layer on or off; Map, Contours and Badges start on, Airflow off.
- 16 words, `211-learn-kitting`:  takes one item per line, such as "rail 1200 x2" or "wire 1.5 BK 37.5 m". 
- 15 words, `211-learn-kitting`: New bar, to buy. For +CAB1 D1, +CAB1 D2, +CAB1 D3, +CAB1 D4, +CAB1 D5
- 15 words, `530-learn-heat-claims`: 2. Find the chips Map, Contours, Airflow and Badges in the strip under the bar.
- 14 words, `211-learn-kitting`: 1. new 2000 mm: +CAB1 R1 840, +CAB1 R2 840; 316 mm left, keep
- 14 words, `211-learn-kitting`: 2. new 2000 mm: +CAB1 R3 840, +CAB1 R4 840; 316 mm left, keep
- 14 words, `211-learn-kitting`: 3. new 2000 mm: +CAB1 R5 840, +CAB1 R6 840; 316 mm left, keep
- 14 words, `530-learn-heat-claims`:  appears in the corner, and four chips appear in the strip under the bar: 
- 13 words, `211-learn-kitting`: 6 bars and 0 reels to buy, 3 lines on the shopping list
- 13 words, `211-learn-kitting`: Fasteners. For 36 holes + 10 % spares; size and length to choose
- 13 words, `530-learn-heat-claims`: 1. Click Heat in the View group of the Cabinet bar, or press 
- 12 words, `120-drawing-standard-click-ticks`: . The title bar now shows · NFPA after the project name.
- 12 words, `211-learn-kitting`: , the last of the four tabs (Cut list, Ferrules, Labels, Kitting).
- 11 words, `211-learn-kitting`: KITTING: WHAT TO TAKE FROM THE SHELF AND WHAT TO BUY
- 11 words, `530-learn-heat-claims`: WHERE IT GETS HOT, AND HOW BIG THE CABINET MUST BE
- 10 words, `120-drawing-standard-click-ticks`: X 196.0 mm  Y 36.0 mm   Home zoom to fit
- 10 words, `120-drawing-standard-click-ticks`: X 160.0 mm  Y 20.0 mm   Home zoom to fit
- 10 words, `211-learn-kitting`: X 200.0 mm  Y 36.0 mm   Home zoom to fit
- 10 words, `211-learn-kitting`:  keeps the settings in the project as one undo step.
- 10 words, `530-learn-heat-claims`: X 200.0 mm  Y 20.0 mm   Home zoom to fit
- 10 words, `530-learn-heat-claims`:  in the View group of the Cabinet bar (or press 
- 10 words, `530-learn-heat-claims`: ; in a narrow window the bar folds Heat into 
- 9 words, `211-learn-kitting`: Counts four checks per wire plus the mounting plate.
- 9 words, `211-learn-kitting`: No wires to cut yet: connect two device pins

### Learning

- 39 words, `21-learning-suite`: Let the model try where a pair of vents or fans goes and how big, read the best few with their before and after numbers, add one to the Vents and fans list, and know what to check next.
- 38 words, `21-learning-suite`: Print the build order as half-page cards for a workshop that cuts wires and drills holes by hand: one card per step, a 1:1 drilling template, wire cut cards, and what is exact and what is a guess.
- 36 words, `21-learning-suite`: Run the 3D heat and airflow model on a hot cabinet, choose its quality, read the verdict and the per-device labels, and know when a result is out of date and why it is an estimate.
- 35 words, `21-learning-suite`: Let the model try moving the hottest devices, and the ones that suffer, along their rails, read what each move wins, apply the moves as one undo step, and know when moving is not enough.
- 35 words, `21-learning-suite`: Open the Kitting tab, read the pick list per build phase, the saw plan with its offcuts and the shopping list, set the stock your shop holds, and write the kitting list as a PDF.
- 35 words, `21-learning-suite`: Step through the wiring in bench order in 3D: both ends and the path lit, the cut, strip, ferrule, label and duct fill for each wire, a saved tick per wire, and a printable list.
- 34 words, `21-learning-suite`: Open the EMC check, preview a repair on a copy, see it before and after on the plate, apply it as one undo step, and know which repairs are left to you by hand.
- 34 words, `21-learning-suite`: Name the hottest day, let the model try the fixes against each other, read the ranked list with its before and after table, preview one in 3D, apply it, and check it with Simulate.
- 34 words, `21-learning-suite`: Work the wire test sheet in wiring order: type the continuity and insulation readings, see a miss raise a punch item itself, find the wires nobody has measured, and write the test sheet PDF.
- 33 words, `21-learning-suite`: Walk the plate in the order the workshop works, tick each step with one big button, raise a problem to the punch list and carry the ticks between a tablet and the desk.
- 32 words, `21-learning-suite`: Two contactors swap two phases to turn the motor both ways. Reverse is dead: find the broken interlock path, repair it, and see why the two contactors can never be on together.
- 31 words, `21-learning-suite`: Read the bench output: what to cut, which ferrule to crimp, which label to stick, and the 1:1 drilling template, all checked for what is exact and what is a guess.
- 31 words, `21-learning-suite`: Measure, move, align and duplicate devices in the 3D view under the Cabinet's own rules, then click a wire and read its routed path, length and how full each duct is.
- 31 words, `21-learning-suite`: See the EMC picture over the mounting plate, switch on the bonding of each drive to the plate and the way each cable shield ends, and read what each mark means.
- 29 words, `21-learning-suite`: Read the cut length of every wire in the wire list, see where it comes from, watch it change when a device moves, and export the wire cut file.
- 29 words, `21-learning-suite`: Place a PLC, its supply, a drive, relays and three terminal strips, read the rails Place all makes, find the hot drive and look at the panel in 3D.
- 28 words, `21-learning-suite`: Colour the wires of a drive page by cable class, read why a wire has its class, fix an unshielded motor cable and set a class by hand.
- 28 words, `21-learning-suite`: Three contactors and a timer start a large motor gently: star first, then delta. Trace the sequence, find the missing path to the delta contactor and repair it.
- 25 words, `21-learning-suite`: Read a tag like =A1+CAB-K1, see how pages are filed by function and location, and let the program renumber devices without breaking a single reference.
- 25 words, `21-learning-suite`: See how devices sit on DIN rails, why Place all lays ducts beside the rows, and how many bars of rail and duct to buy.
- 23 words, `21-learning-suite`: Switch on Route all, read the length of every wire and the fill of every duct, and see a move change the numbers.
- 22 words, `21-learning-suite`: Save, find your unsaved work after a crash, open a file read-only, and know what happens with older, newer and damaged files.
- 22 words, `21-learning-suite`: Drag devices onto the plate, turn one, read an overlap, undo it and let Place all fill the plate by the rules.
- 22 words, `21-learning-suite`: Enter the inside size and door of the cabinet and let KoperCAD check device depth and the room the open door needs.
- 22 words, `21-learning-suite`: Add a PLC card, send a button and a lamp to it, read the addresses and try both in the live view.
- 21 words, `21-learning-suite`: After the first hour: real machine circuits, one step harder each time, each one a circuit you trace, break and repair.
- 21 words, `21-learning-suite`: Lay out the plate, size and route, check door and heat, then drawings and labels for the workshop, 3D and commissioning.
- 21 words, `21-learning-suite`: Turn on the heat view, find the hot spot, move the transformer, and see what the heat means for the enclosure.
- 21 words, `21-learning-suite`: Open the cabinet in 3D, orbit, zoom and pan it, use the view cube and see how deep each device stands.
- 20 words, `21-learning-suite`: Anyone new: place parts, wire them, give them a supply, draw a motor starter, follow its contacts, watch it run.
- 20 words, `21-learning-suite`: Guess what the pump starter will do, then try it in the live view and see whether you were right.
- 20 words, `21-learning-suite`: Export a dimensioned plate drawing with a numbered drill table for the workshop, and see it follow a turned device.
- 20 words, `21-learning-suite`: Walk the commissioning checks, pass and fail them with a value, and keep a punch list until it is clear.
- 18 words, `21-learning-suite`: Put a breaker, a button and a relay on the page with the wheel and the fast keys.
- 18 words, `21-learning-suite`: Builds the cabinet: layout, rails and ducts, routing, cut lengths, plate drawing, labels, heat, 3D, PLC and commissioning.
- 18 words, `21-learning-suite`: Turn a motor starter into a macro, place it on another page and share it as a file.
- 18 words, `21-learning-suite`: The pump starter drops out the moment you let go of start. Find the fault and fix it.
- 18 words, `21-learning-suite`: Export the four kinds of labels to a CSV for your label printer, with the command line tool.
- 18 words, `21-learning-suite`: Step through the cabinet in 3D in the order a workshop builds it: rails, ducts, devices, then wires.
- 17 words, `21-learning-suite`: Issues and hands over: generated pages, wire list and parts list, revisions, PDF, parts library, spreadsheets, DXF.
- 16 words, `21-learning-suite`: Draws the schematics: after the first hour, terminals, signals across pages, cables, wire numbers, checks, blocks.
- 15 words, `21-learning-suite`: The pump keeps running when you press stop. Find out why and repair the drawing.
- 15 words, `21-learning-suite`: Set the auto connections, give a key your own command and pick the drawing standard.
- 14 words, `21-learning-suite`: Give a motor feeder its supply with a row of feeds and an earth.
- 14 words, `21-learning-suite`: Jump between the coil and its contacts, and add a contact on another page.
- 14 words, `21-learning-suite`: Switch on the live view, press the start button and read what lights up.
- 13 words, `21-learning-suite`: Draw the power circuit and the control circuit of a direct-on-line motor starter.
- 13 words, `21-learning-suite`: Draw wires from a pin, and bend them in the two wiring modes.
- 13 words, `21-learning-suite`: You press start and nothing happens. Follow the power and repair the break.
- 12 words, `21-learning-suite`: Short lessons on a small drawing, with a pointer on the control.
- 11 words, `21-learning-suite`: Where it gets hot, and how big the cabinet must be
- 11 words, `21-learning-suite`: Kitting: what to take from the shelf and what to buy
- 10 words, `21-learning-suite`: Lessons are early access: a step may still be rough.
- 10 words, `21-learning-suite`: Print for a workshop that cuts and drills by hand
- 10 words, `21-learning-suite`: Work cards: drill, cut and tick off at the bench
- 9 words, `21-learning-suite`: Edit in 3D, follow a wire through the ducts
- 9 words, `21-learning-suite`: Where a vent and fan should go: cooling advice

### Live

- 25 words, `1800-defects-card-opens-and-injects`: Not a safety function and not a compliance result: it shows what the drawn circuit does when one thing breaks, not how likely that is.
- 21 words, `1800-defects-card-opens-and-injects`: Live potentials: the plant is at rest, break contacts closed. Click a pole badge to switch the device, hold a pushbutton
- 15 words, `1800-defects-card-opens-and-injects`: Nothing differs yet. Operate the circuit: a defect often shows only when it is used.
- 13 words, `1581-controller-live-radio-hoist`: Hold. Press the top zone for step 1, slide down for step 2
- 13 words, `1800-defects-card-opens-and-injects`: None saved. Record one, or write one in a file and import it.
- 12 words, `1581-controller-live-radio-hoist`: radio-1: 9 signals drive nothing (BRIDGE_FWD_SLOW, BRIDGE_FWD_FAST, BRIDGE_BACK_SLOW, BRIDGE_BACK_FAST and 5 more)
- 12 words, `1584b-controller-pendant-joystick-1920`: generic-pendant-3x2: 13 signals drive nothing (HOIST_UP_SLOW, HOIST_UP_FAST, HOIST_DN_SLOW, HOIST_DN_FAST and 9 more)
- 10 words, `1581-controller-live-radio-hoist`: X 180.0 mm  Y 148.0 mm   Home zoom to fit
- 10 words, `1584b-controller-pendant-joystick-1920`: X 60.0 mm  Y 100.0 mm   Home zoom to fit
- 10 words, `1800-defects-card-opens-and-injects`: X 104.0 mm  Y 16.0 mm   Home zoom to fit
- 10 words, `1800-defects-card-opens-and-injects`: X 384.0 mm  Y 200.0 mm   Home zoom to fit
- 10 words, `1800-defects-card-opens-and-injects`: X 12.0 mm  Y 8.0 mm   Home zoom to fit
- 10 words, `1800-defects-card-opens-and-injects`: X 376.0 mm  Y 136.0 mm   Home zoom to fit
- 9 words, `1584b-controller-pendant-joystick-1920`: generic-joystick-2axis: 4 signals drive nothing (HOIST_UP_1, HOIST_DN_1, BRIDGE_FWD_1, BRIDGE_BACK_1)

### Macros

- 39 words, `24-starter-macros`: PLC CPU on an Ethernet switch with two network devices. -T1 is the network port of a frequency converter (rename it to the converter's tag so both symbols share one device); -A3 is the port of an operator panel
- 39 words, `24-starter-macros`: Wiring example: two-channel emergency stop on a safety relay with a reset button and two contactor coils. The category and performance level depend on the relay type and the whole circuit; nothing here is validated as a 
- 34 words, `24-starter-macros`: Bridge travel with two end carriage motors on one frequency converter: breaker, line contactor, converter, a motor protection breaker for each motor, forward and reverse commands through end limits and the motor thermal 
- 34 words, `24-starter-macros`: Placed Three-phase supply, L1 L2 L3 N PE fed · now wire its outputs to the starter · Takes -F1, -F2, -Q1 · 4 points to connect to a supply (see the Issues list)
- 31 words, `24-starter-macros`: Hoist on a frequency converter: breaker, line contactor, converter with braking resistor, motor, brake released by the converter's relay output with a feedback contact, up and down commands through end limits
- 30 words, `24-starter-macros`: Placed Crane hoist up/down · Takes -B1, -B2, -F1, -F2, -F3, -K1, -K2, -M1, -S1, -S2, -S3, -Y1 · 3 points to connect to a supply (see the Issues list)
- 29 words, `24-starter-macros`: Level control for a duty pump and a standby pump with float switches on 24 V DC: dry-run lockout, start and stop level, high level starts the second pump
- 27 words, `24-starter-macros`: Placed Star-delta starter · Takes -F1, -F2, -F3, -K1, -K2, -K3, -K4, -M1, -S1, -S2 · 3 points to connect to a supply (see the Issues list)
- 27 words, `24-starter-macros`: Placed Motor starter reversing · Takes -F1, -F2, -F3, -K1, -K2, -M1, -S1, -S2, -S3 · 3 points to connect to a supply (see the Issues list)
- 27 words, `24-starter-macros`: Bridge or trolley travel on a frequency converter: breaker, line contactor, converter, motor, brake released by the converter's relay output, forward and reverse commands through end limits
- 26 words, `24-starter-macros`: 24 V PLC card symbols (add the card via Add PLC card): four digital inputs with a push button, stop button, key switch and float switch
- 26 words, `24-starter-macros`: Placed Motor with brake · Takes -F1, -F2, -F3, -K1, -M1, -S1, -S2, -Y1 · 3 points to connect to a supply (see the Issues list)
- 25 words, `24-starter-macros`: Pump motor on a frequency converter behind a motor protection breaker, run command through a relay contact, fault relay output left open, screened motor cable
- 25 words, `24-starter-macros`: Placed Single-phase supply with 24 V · Takes -F1, -F2, -F3, -G1, -X1 · 5 points to connect to a supply (see the Issues list)
- 25 words, `24-starter-macros`: Placed Pump with float switch · Takes -B1, -F1, -K1, -M1, -Q1, -S1 · 4 points to connect to a supply (see the Issues list)
- 25 words, `24-starter-macros`: Placed Emergency stop with safety relay · Takes -K1, -K2, -K3, -S1, -S2 · 4 points to connect to a supply (see the Issues list)
- 24 words, `24-starter-macros`: Placed Heater with thermostat · Takes -B1, -B2, -E1, -F1, -F2, -K1 · 3 points to connect to a supply (see the Issues list)
- 23 words, `24-starter-macros`: Placed Soft starter with bypass · Takes -F1, -K1, -M1, -T1 · 3 points to connect to a supply (see the Issues list)
- 23 words, `24-starter-macros`: Placed Contactor with indication · Takes -F1, -H1, -H2, -K1, -S1 · 2 points to connect to a supply (see the Issues list)
- 23 words, `24-starter-macros`: Placed PLC network with switch · Takes -A1, -A2, -A3, -T1 · 3 points to connect to a supply (see the Issues list)
- 22 words, `24-starter-macros`: Placed Power supply 24 V · Takes -F1, -F2, -G1 · 4 points to connect to a supply (see the Issues list)
- 22 words, `24-starter-macros`: Placed Start/stop latch · Takes -F1, -H1, -K1, -S1, -S2 · 1 point to connect to a supply (see the Issues list)
- 22 words, `24-starter-macros`: Placed PLC input card (symbols only: use Draw > Add PLC card for addresses) · Takes -A1, -A2, -B1, -S1, -S2, -S3
- 21 words, `24-starter-macros`: Main switch and main contactor released by a chain of emergency stop stations, with start button, self-hold and a mains-on lamp
- 21 words, `24-starter-macros`: Control transformer with fuses and breaker, and an AC/DC power supply that makes 24 V DC from the 48 V line
- 21 words, `24-starter-macros`: Hoist motor with up and down contactors, hold-to-run buttons, end limit switches, emergency stop and a brake released by either contactor
- 21 words, `24-starter-macros`: Placed Frequency converter drive · Takes -F1, -M1, -T1 · 2 points to connect to a supply (see the Issues list)
- 21 words, `24-starter-macros`: Two-channel emergency stop on a safety relay with a reset button, a feedback loop through two contactors and two contactor coils
- 20 words, `24-starter-macros`: Placed E-stop chain · Takes -K1, -S1, -S2 · 3 points to connect to a supply (see the Issues list)
- 20 words, `24-starter-macros`: Reversing motor feeder with a motor protection breaker, forward and reverse contactors and an electrical interlock in each coil path
- 19 words, `24-starter-macros`: Incremental encoder on the hoist motor wired to the encoder input of the frequency converter for closed-loop speed control
- 19 words, `24-starter-macros`: Contactor switched by a selector, with a green running lamp and a red stopped lamp from its auxiliary contacts
- 18 words, `24-starter-macros`: Symbols only: no addresses or I/O list until you add a PLC card (Draw > Add PLC card)
- 17 words, `24-starter-macros`: 230 V incomer with RCBO, a 24 V DC supply with output breaker and a service socket
- 17 words, `24-starter-macros`: Pump motor on a motor protection breaker and contactor, started by hand or by a float switch
- 16 words, `24-starter-macros`: Motor feeder with a motor protection breaker and a contactor, coil left open for your control
- 16 words, `24-starter-macros`: Single-phase control transformer fed from two phases through fuses, with a breaker on the secondary side
- 16 words, `24-starter-macros`: Three-phase incomer with main switch, outgoing breaker, control circuit breaker, and N and PE terminal symbols
- 13 words, `24-starter-macros`: Star-delta motor starter with line, delta and star contactors and an on-delay timer
- 13 words, `24-starter-macros`: Motor soft starter behind fuses with a bypass contactor across its power section
- 13 words, `24-starter-macros`: Takes -B1, -B2, -F1, -F2, -F3, -K1, -K2, -M1, -S1, -S2, -S3, -Y1
- 12 words, `24-starter-macros`: Direct-on-line motor starter whose holding brake is released through a contactor contact
- 11 words, `24-starter-macros`: Takes -F1, -F2, -F3, -K1, -K2, -K3, -K4, -M1, -S1, -S2
- 11 words, `24-starter-macros`: Two emergency stop buttons in series that drop a safety relay
- 11 words, `24-starter-macros`: Switched-mode power supply behind a two-pole breaker, with an output breaker
- 11 words, `24-starter-macros`: Reversing motor starter with forward and reverse buttons, interlock and self-hold
- 11 words, `24-starter-macros`: Relay latched by start and stop buttons with a run lamp
- 11 words, `24-starter-macros`: Three-phase heater switched by a thermostat with an independent high-limit thermostat
- 10 words, `24-starter-macros`: Direct-on-line motor starter with start and stop buttons and self-hold
- 10 words, `24-starter-macros`: Takes -F1, -F2, -F3, -K1, -K2, -M1, -S1, -S2, -S3
- 10 words, `24-starter-macros`: Motor fed from a frequency converter behind a three-pole breaker
- 9 words, `24-starter-macros`: Control supply 48 V AC and 24 V DC
- 9 words, `24-starter-macros`: Takes -F1, -F2, -F3, -K1, -M1, -S1, -S2, -Y1

### Options

- 39 words, `170-options-ghost-and-columns`: 22 connections between pins + 6 supply and PE leads = 28 wires to cut; 56 wire labels (one per wire end); 30 wire pieces drawn (a connection can be drawn in several pieces, and supply feeds count too).
- 16 words, `172-options-column-in-exports`: Exported …/KoperCAD/machine-options.pdf, with a Design checks page at the end (4 errors · 14 warnings remain)
- 12 words, `170-options-ghost-and-columns`: 22 CONNECTIONS BETWEEN DEVICE PINS (SUPPLY AND PE LEADS: SEE WIRE PRODUCTION)
- 10 words, `170-options-ghost-and-columns`: X 96.0 mm  Y 76.0 mm   Home zoom to fit
- 10 words, `170-options-ghost-and-columns`: X 40.0 mm  Y 40.0 mm   Home zoom to fit
- 9 words, `172-options-column-in-exports`: Cooling fan on: adds 1 page and 2 devices

### Palette

- 9 words, `54-palette-live-view`: "Motor starter (example)" has changes that are not saved.

### Parts

- 81 words, `701-parts-bmecat-etim-prices`: Maker              files parts  new updated  same no size no ETIM no price flags Example Maker          1     2    2       0     0       1       1        0     0  no size: no width, height or depth. no ETIM: no ETIM clas
- 26 words, `1581-assign-all-never-matches-wrong-kind`: Tick the devices that get a part and pick the part. Only parts that are sure are ticked; a part of another kind is never offered.
- 25 words, `61-parts-table-audit`: 1 order number appears in more than one row (2 rows). The catalog keeps one entry per maker and order number: choose what it gets.
- 25 words, `61-parts-table-audit`: 3 order numbers appear in more than one row (6 rows). The catalog keeps one entry per maker and order number: choose what it gets.
- 24 words, `2419-pin-offer-keep-1920`: The terminals of the part on -K1 are not named like the usual pins of its symbol, so they cannot be matched to them.
- 24 words, `61-parts-table-audit`: Parts import: price-list-nl.csv is a file, not a folder. To import one CSV or Excel file use File > Import... > Import parts table.
- 20 words, `21-parts-table-import`: Say what each column is. Nothing is added until you choose Add; what the file does not say stays unknown.
- 20 words, `61-parts-table-audit`: the price is for "per meter", not for pieces; the price is left out because it could not be compared
- 20 words, `61-parts-table-audit`: 9 rows: 5 new, 0 changed, 3 not imported, 1 repeated order number (1 row not used), 2 with warnings
- 20 words, `61-parts-table-audit`: not-text.csv: this does not look like a text file; if it is a spreadsheet, save it as .xlsx or CSV
- 20 words, `61-parts-table-audit`: 44 rows: 40 new, 0 changed, 1 not imported, 3 repeated order numbers (3 rows not used), 1 with warnings
- 19 words, `2419-pin-offer-keep-1920`: The part is shared by every device that uses it, so its terminals are renamed for all of them.
- 19 words, `701-parts-bmecat-etim-prices`: Contactor S00, 3 pole, AC-3 9 A / 4 kW at 400 V, coil 24 V DC, 1 NO
- 19 words, `701-parts-bmecat-etim-prices`: Contactor S00, 3 pole, AC-3 9 A / 4 kW at 400 V, coil 230 V AC, 1 NO
- 19 words, `701-parts-bmecat-etim-prices`: Contactor S00, 3 pole, AC-3 12 A / 5.5 kW at 400 V, coil 24 V DC, 1 NO
- 18 words, `61-parts-table-audit`: Column "Spoel" has no unit in its header or in some cells: those values are taken as V.
- 17 words, `1700-parts-database-import`: 4 part(s) read: 4 new, 0 would replace a catalog entry, 0 already there as they are.
- 17 words, `1700-parts-database-import`: One name per maker. Type a name next to a code to merge it with a maker.
- 17 words, `61-parts-table-audit`: header "Spoel" names coil voltage, no unit in the header, V assumed unless a cell says otherwise
- 17 words, `61-parts-table-audit`: 5 rows: 4 new, 0 changed, 1 repeated order number (1 row not used), 1 with warnings
- 17 words, `61-parts-table-audit`: header "Width" names width (mm), no unit in the header, mm assumed unless a cell says otherwise
- 16 words, `21-parts-table-import`: ETIM: "12345" is not an ETIM class (EC and six digits); the class is left unknown
- 16 words, `61-parts-table-audit`: Prijsdatum: "31-02-2026" is not a date; the price is dated with the day of the import
- 16 words, `61-parts-table-audit`: Parts from price-list-nl.csv: 4 added, 0 updated, 0 not imported, 1 repeated row not separate parts
- 15 words, `21-parts-table-import`: Breite mm: 45000 is not a possible size in mm; the value is left unknown
- 15 words, `701-parts-bmecat-etim-prices`: Installed etim-test: 4 features, 0 values, 2 units, 0 classes. Import the maker file again.
- 14 words, `701-parts-bmecat-etim-prices`: No 3D model. The part is drawn as a box from its catalog size.
- 14 words, `701-parts-bmecat-etim-prices`: 0 of 0 terminals have a position. Wires end on them when they do.
- 13 words, `1700-parts-database-import`: Only your own files are read. Nothing is added until you choose Add.
- 13 words, `61-parts-table-audit`: Example Maker NL-100 is in rows 4 and 6; row 6 is used
- 13 words, `61-parts-table-audit`: Example Maker NL-100 is in rows 4 and 6; row 4 is used
- 13 words, `61-parts-table-audit`: Example Maker LG-009 is in rows 10 and 11; row 11 is used
- 13 words, `61-parts-table-audit`: Example Maker LG-019 is in rows 20 and 21; row 21 is used
- 13 words, `61-parts-table-audit`: Example Maker LG-029 is in rows 30 and 31; row 31 is used
- 13 words, `701-parts-bmecat-etim-prices`: Thermal overload relay S00, 2.8 to 4 A, 1 NO + 1 NC
- 13 words, `701-parts-bmecat-etim-prices`: Thermal overload relay S00, 3.5 to 5 A, 1 NO + 1 NC
- 13 words, `701-parts-bmecat-etim-prices`: Thermal overload relay S00, 7 to 10 A, 1 NO + 1 NC
- 12 words, `1700-parts-database-import`: Kept as technical data: description_nl, technical_text, eplan.macro_path, aux_contacts_nc, aux_contacts_no, coil_current_type, coil_voltage_v, poles
- 12 words, `21-parts-table-import`: Schütz 9 A  ·  Schütz 12 A  ·  Schütz 500 mA …
- 12 words, `21-parts-table-import`: 5 rows: 4 new, 0 changed, 1 not imported, 2 with warnings
- 12 words, `61-parts-table-audit`: Prijsdatum: "05-10-2026" read as day.month.year; month first would be a different date
- 12 words, `61-parts-table-audit`: Prijsdatum: "12-10-2026" read as day.month.year; month first would be a different date
- 12 words, `61-parts-table-audit`: the same part is also in row 4; this row is used
- 12 words, `61-parts-table-audit`: the same part is also in row 6; this row is used
- 12 words, `61-parts-table-audit`: the same part is also in row 10; this row is used
- 12 words, `61-parts-table-audit`: the same part is also in row 20; this row is used
- 12 words, `61-parts-table-audit`: the same part is also in row 30; this row is used
- 12 words, `701-parts-bmecat-etim-prices`: Contactor relay S00, 2 NO + 2 NC, coil 24 V DC
- 11 words, `1700-parts-database-import`: 1 code is not known and added under the code itself.
- 11 words, `61-parts-table-audit`: the same part is in row 6, which is used instead
- 11 words, `61-parts-table-audit`: the same part is in row 4, which is used instead
- 11 words, `61-parts-table-audit`: Started on the sheet with the order numbers; change it here.
- 11 words, `61-parts-table-audit`: the same part is in row 11, which is used instead
- 11 words, `61-parts-table-audit`: the same part is in row 21, which is used instead
- 11 words, `61-parts-table-audit`: the same part is in row 31, which is used instead
- 11 words, `61-parts-table-audit`: Price: "abc" is not a price; the price is left unknown
- 10 words, `1700-parts-database-import`: X 396.0 mm  Y 216.0 mm   Home zoom to fit
- 10 words, `21-parts-table-import`: header "Nennstrom" names rated current, unit A from the header
- 10 words, `21-parts-table-import`: header "Verlustleistung" names power loss, unit W from the header
- 10 words, `21-parts-table-import`: header "Breite" names width (mm), unit mm from the header
- 10 words, `21-parts-table-import`: header "Höhe" names height (mm), unit mm from the header
- 10 words, `21-parts-table-import`: header "Tiefe" names depth (mm), unit mm from the header
- 10 words, `21-parts-table-import`: Parts from price-list-de.csv: 4 added, 0 updated, 1 not imported
- 10 words, `21-parts-table-import`: X -52.0 mm  Y 224.0 mm   Home zoom to fit
- 10 words, `2419-pin-offer-keep-1920`: X 52.0 mm  Y 52.0 mm   Home zoom to fit
- 10 words, `2419-pin-offer-keep-1920`: X 128.0 mm  Y 80.0 mm   Home zoom to fit
- 10 words, `61-parts-table-audit`: header "Bedrijfsstroom" names rated current, unit A from the header
- 10 words, `61-parts-table-audit`: header "Warmteverlies" names power loss, unit W from the header
- 10 words, `61-parts-table-audit`: X -52.0 mm  Y 268.0 mm   Home zoom to fit
- 10 words, `61-parts-table-audit`: header "In" names rated current, unit A from the header
- 10 words, `61-parts-table-audit`: header "Pv" names power loss, unit W from the header
- 10 words, `701-parts-bmecat-etim-prices`: Pick a part in the list to see its data.
- 10 words, `701-parts-bmecat-etim-prices`: Miniature circuit breaker 1 pole, C 6 A, 6 kA
- 10 words, `701-parts-bmecat-etim-prices`: Miniature circuit breaker 1 pole, C 16 A, 6 kA
- 10 words, `701-parts-bmecat-etim-prices`: Miniature circuit breaker 3 pole, C 16 A, 6 kA
- 10 words, `701-parts-bmecat-etim-prices`: Parts import: 2 read, 2 new, catalog now 2 parts
- 9 words, `1700-parts-database-import`: Reading the parts. You can keep this window open.
- 9 words, `1700-parts-database-import`: Parts from invented-articles.eod: 4 added, 0 updated, 0 rejected
- 9 words, `61-parts-table-audit`: Contactor 9 A  ·  Hulprelais 6 A  ·  Contacto…
- 9 words, `61-parts-table-audit`: ok  ·  units in cells  ·  negative loss, text…
- 9 words, `701-parts-bmecat-etim-prices`: Motor protection circuit breaker S00, 2.8 to 4 A
- 9 words, `701-parts-bmecat-etim-prices`: Motor protection circuit breaker S00, 3.5 to 5 A
- 9 words, `701-parts-bmecat-etim-prices`: Motor protection circuit breaker S00, 7 to 10 A
- 9 words, `701-parts-bmecat-etim-prices`: Cylindrical fuse holder 10 x 38 mm, 3 pole

### Regressions

- 42 words, `20-regressions-critic`: The report assists the engineer: the rules are a subset of the standard, not a certification. IEC 60204-1 clause numbers are unverified (no licensed copy was used) and verdicts marked planning value rest on numbers not y

### Reports

- 50 words, `08-autopilot-groups`: 6 wire lengths are estimates: the wire comes in from outside the drawing (a supply lead with no device at the other end). Set the cabinet entry and the wire length outside it (Cabinet > Routes): the lead is then routed t
- 41 words, `49-handover-safety-files`: Handover package: 10 files in /tmp/kopercad-test-user-31925/home/Documents/KoperCAD/Motor starter (example) handover 2026-11-03. 0 Contents.pdf says what is exact and what is an estimate. Left out: 7 Commissioning record
- 41 words, `49-handover-safety-files`: Handover package: 10 files in …/KoperCAD/Motor starter (example) handover 2026-11-03. 0 Contents.pdf says what is exact and what is an estimate. Left out: 7 Commissioning record.pdf (commissioning has not started), 10 Re
- 39 words, `58-single-line`: missing: breaking capacity ka on the device or its part, trip curve (B, C, D) on the device, load current a on the wire (the device rating is used instead), installation method on the wire (method C is assumed)
- 36 words, `1791-ferrule-crimp-cut-labels`: Colour, contact length, overall length, crimp profile and tool die per cross-section. The default is the DIN 46228-4 colour code as commonly sold; makers differ, above 16 mm² in particular. Edit it to match your stock.
- 35 words, `08-autopilot-groups`: 21 of 25 wires to cut have no cross-section: they get no ferrule and sit in one group. Autopilot proposes them from the protective devices and the ampacity tables (planning values), with a reason each
- 34 words, `49-handover-safety-files`: Workshop pack: print scale check, drilling template +CAB (A4), drill plan +CAB, plate +CAB on one sheet for a print shop, labels, kitting list, work cards, bench packet in …/Motor starter (example) handover 2026-11-03/Wo
- 33 words, `08-autopilot-groups`: 21 sizes, 21 colours, 0 wire numbers, 0 cables and 1 cabinet entry point can be filled in from the drawing. Nothing is written until you accept; one Undo takes it all back.
- 29 words, `08-autopilot-groups`: 6 lengths are estimates and print with a ~: the wire is not routed through a duct, or a device has no pin position. Measure those on the plate.
- 29 words, `08-autopilot-groups`: Not set: 21 of 25 wires to cut have no size (no strip length, no ferrule); 21 of 25 wires to cut have no colour (blank in the list).
- 27 words, `08-autopilot-groups`: Cut length = route + 10 mm per end for a ferrule (20 mm from 25 mm² for a lug). Change the allowances under Cabinet > Routes.
- 26 words, `08-autopilot-groups`: Sizes use planning values: the built-in ampacity tables are not checked against the standard. Read the clauses in your licensed copy before relying on a number.
- 23 words, `08-autopilot-groups`: Unverified: the IEC clause numbers in the reasons are not checked against a licensed copy. Check them in your copy of the standard.
- 23 words, `49-handover-safety-files`: 1 warning remains. Handing over anyway lists them on the contents page and in the drawing set, so the customer would see them.
- 18 words, `308-problem-highlight`: The DIN rail with =A1+CAB-S0, =A1+CAB-S1 on the plate of +CAB runs under a duct for 535 mm
- 15 words, `08-autopilot-groups`: 53 of 101 checks: four per wire (number, size, colour, known length) plus the plate.
- 13 words, `49-handover-safety-files`: The door of +CAB needs 100 mm more room in front when open
- 12 words, `08-autopilot-groups`: 25 wires in 5 groups, 9.5 m of hookup wire to pull
- 11 words, `08-autopilot-groups`: Cable =A1-W1: 12.0 m to buy; 4 cores cut from it
- 10 words, `08-autopilot-groups`: X 140.0 mm  Y 156.0 mm   Home zoom to fit
- 10 words, `08-autopilot-groups`: 6 lengths are estimates (~): route the wires through ducts
- 10 words, `308-problem-highlight`: X -64.0 mm  Y 64.0 mm   Home zoom to fit
- 10 words, `308-problem-highlight`: X -64.0 mm  Y 100.0 mm   Home zoom to fit
- 10 words, `308-problem-highlight`: X -64.0 mm  Y 1540.0 mm   Home zoom to fit
- 10 words, `49-handover-safety-files`: X 160.0 mm  Y 104.0 mm   Home zoom to fit
- 10 words, `49-handover-safety-files`: Exported bench packet (6 lengths are estimates): …/Workshop/0 Bench packet.pdf
- 10 words, `58-single-line`: calculated from 2.2 kW with typical efficiency and power factor

### Revisions

- 14 words, `09-revision-lock`: Freezes the pages as they are now and keeps a copy to compare with.
- 10 words, `09-revision-lock`: X 132.0 mm  Y 204.0 mm   Home zoom to fit

### Schematic

- 38 words, `2412-pin-names-company-templates`: Your own pin names per device class, as TOML. A class replaces the built-in names for the symbol functions it lists; Check > Issues then flags devices that differ (kind "Pin names"). Leave empty for the built-in names.
- 37 words, `17100b-circuit-library-mounting-view-1920`: Placed Hoist two-speed with rectifier brake · Takes -B1, -B2, -B3, -B4, -F1, -K1, -K2, -K3, -K4, -M1, -Q1, -S1, -S2, -S3, -S4, -V1, -Y1 · 4 points to connect to a supply (see the Issues list)
- 36 words, `07-autowire-pin-kind`: Not wired: -M1 U1 to -K1 A2 (power to control). Draw it with W if you mean it · Click to place Three-phase motor as -M2 · R rotate · X mirror · Esc to stop
- 32 words, `07-autowire-pin-kind`: Connected 1 facing pin pair that belong together (Ctrl+Z undoes them with the drop) · Click to place Contactor coil as -K2 · R rotate · X mirror · Esc to stop
- 29 words, `17b-inspector-sheet-cover-1920`: Corners: click a pin or wire to finish, empty space for a corner · R flips the bend · Backspace undo · Esc or right-click cancels · Shift+W lines
- 27 words, `17100b-circuit-library-mounting-view-1920`: Spring brake fed from the mains through a bridge rectifier and a contactor that switches the AC side and the DC side, so the brake falls quickly
- 27 words, `17100b-circuit-library-mounting-view-1920`: Two-speed pole-change bridge or trolley travel with forward and reverse contactors, slow and fast speed contactors, brake motor with rectifier, thermal contact, end limits and slow-down limits
- 26 words, `17100b-circuit-library-mounting-view-1920`: Two-speed pole-change hoist with up and down contactors, slow and fast speed contactors, a spring brake released through a rectifier, motor thermal contact and end limits
- 25 words, `34-reopen-fit`: Placed Motor starter DOL · Takes -F1, -F2, -F3, -K1, -M1, -S1, -S2 · 4 points to connect to a supply (see the Issues list)
- 20 words, `17100b-circuit-library-mounting-view-1920`: Single-speed hoist with up and down contactors, a spring brake released through a rectifier, motor thermal contact and end limits
- 18 words, `17100b-circuit-library-mounting-view-1920`: Takes -B1, -B2, -B3, -B4, -F1, -K1, -K2, -K3, -K4, -M1, -Q1, -S1, -S2, -S3, -S4, -V1, -Y1
- 14 words, `17b-inspector-sheet-cover-1920`: Blank is unknown: the checks stay silent, nothing is guessed. Results: Check > Calculations.
- 14 words, `17b-inspector-sheet-cover-1920`: Click a pin, junction or wire to start a wire · Esc to stop
- 13 words, `2412-pin-names-company-templates`: Saved in the project file. Every rule is explicit; undo takes one back.
- 13 words, `2412-pin-names-company-templates`: Pages the project adds and keeps current. Deleting one switches it off here.
- 11 words, `2412-pin-names-company-templates`: Press W to connect pins · click one pin, then another
- 11 words, `648-references-hide`: Cross references of this page hidden on screen (they still print)
- 10 words, `07-autowire-pin-kind`: X 160.0 mm  Y 68.0 mm   Home zoom to fit
- 10 words, `07-autowire-pin-kind`: X 160.0 mm  Y 100.0 mm   Home zoom to fit
- 10 words, `17b-inspector-sheet-cover-1920`: X 328.0 mm  Y 228.0 mm   Home zoom to fit
- 10 words, `17b-inspector-sheet-cover-1920`: X 48.0 mm  Y 72.0 mm   Home zoom to fit
- 10 words, `2412-pin-names-company-templates`: X 132.0 mm  Y 72.0 mm   Home zoom to fit
- 10 words, `2412-pin-names-company-templates`: X 224.0 mm  Y 124.0 mm   Home zoom to fit
- 10 words, `34-reopen-fit`: X 48.0 mm  Y 92.0 mm   Home zoom to fit
- 10 words, `34-reopen-fit`: X 384.0 mm  Y 112.0 mm   Home zoom to fit
- 10 words, `648-references-hide`: X 188.0 mm  Y 76.0 mm   Home zoom to fit
- 10 words, `648-references-hide`: X 84.0 mm  Y 64.0 mm   Home zoom to fit
- 9 words, `17100b-circuit-library-mounting-view-1920`: Pick a macro to see what it will do.

### Simulate

- 26 words, `1765c-closed-winch-load-cell-lost-1920`: brake closed · 0 rpm · 0 N m · 0.0 A · 0.0 m/min · 0.00 m · load 83 % of rated, lifting inhibited
- 19 words, `1738c-closed-winch-pair-property-section`: A comma is a decimal comma: 1,5 means 1.5. A comma before three digits (5,000) is refused: type 5000.
- 15 words, `1704-hoist-faults`: 0 rpm · off · brake closed · 0.00 m · 0.0 A · !
- 14 words, `1765c-closed-winch-load-cell-lost-1920`: The load cell signal is gone: the load is unknown, so lifting is inhibited.
- 13 words, `1704-hoist-faults`: 0 rpm · off · brake closed · 0.00 m · 0.0 A
- 11 words, `1765c-closed-winch-load-cell-lost-1920`: =A1+CAB-STOA pulled in · =A1+CAB-STOB pulled in · =A1+CAB-ENC pulled in
- 11 words, `1791b-travel-sheet-and-3d-1920`: =A1+FLD-M1: wind 8 m/s pushes with 1382 N · brake closed
- 10 words, `1704-hoist-faults`: X 252.0 mm  Y 144.0 mm   Home zoom to fit
- 10 words, `1704-hoist-faults`: • Supply voltage is low (below 90 % of rated)
- 10 words, `1730c-hoist-3d-lifting-1920`: X 384.0 mm  Y 4.0 mm   Home zoom to fit
- 10 words, `1730c-hoist-3d-lifting-1920`: =A1+FLD-M1: hook +0.00 m from its start, 0.00 m/s, standing
- 10 words, `1738c-closed-winch-pair-property-section`: X 236.0 mm  Y 200.0 mm   Home zoom to fit
- 10 words, `1738c-closed-winch-pair-property-section`: Torque before the brake opens (none, fixed, load cell, memory)
- 10 words, `1738c-closed-winch-pair-property-section`: Ground level (m above the hook start; blank: hangs free)
- 10 words, `1760-hoist-3d-travel-limits`: X 152.0 mm  Y 80.0 mm   Home zoom to fit
- 10 words, `1765c-closed-winch-load-cell-lost-1920`: X 24.0 mm  Y 8.0 mm   Home zoom to fit
- 10 words, `1791b-travel-sheet-and-3d-1920`: X 64.0 mm  Y 72.0 mm   Home zoom to fit
- 10 words, `1791b-travel-sheet-and-3d-1920`: X 228.0 mm  Y 20.0 mm   Home zoom to fit
- 9 words, `1738c-closed-winch-pair-property-section`: Centre of gravity, from the middle towards winch A

### Start

- 13 words, `210-about-update-check`: Graphics: llvmpipe (LLVM 20.1.2, 256 bits) (Vulkan, driver llvmpipe Mesa 25.2.8-0ubuntu0.24.04.4 (LLVM 20.1.2))
- 13 words, `210-about-update-check`: Run the new installer over this copy; your projects stay as they are
- 11 words, `01-start-and-example`: This is an example. Edit freely; Ctrl+S keeps your own copy.
- 11 words, `210-about-update-check`: or press Check for updates (asks GitHub for the release list).
- 10 words, `01-start-and-example`: Ctrl K finds every command · F10 opens the menu
- 10 words, `01-start-and-example`: X 440.0 mm  Y 268.0 mm   Home zoom to fit
- 10 words, `210-about-update-check`: Works offline. Nothing is sent unless you use Ask Claude
- 9 words, `210-about-update-check`: (your question goes to Anthropic with your own key)

### Terminals

- 28 words, `7375-terminal-right-click-1920`:  joins the two terminals electrically, so one signal feeds both without extra wire. There is no key for this, only the right-click menu. Afterwards the drawer gives terminal 
- 24 words, `7375-terminal-right-click-1920`: . KoperCAD proposes a terminal on every wire that runs between a device in the cabinet (+CAB) and a device in the field (+FLD).
- 20 words, `7375-terminal-right-click-1920`: You will let KoperCAD place and number the terminals for you, instead of typing each one. At the end strip 
- 17 words, `7375-terminal-right-click-1920`:  holds seven numbered terminals, two of them bridged, and the earth terminal sits on its own strip 
- 14 words, `70-set-location-and-strips-1920`: 1 page is in no group or cabinet. Their devices say where they belong.
- 13 words, `70-set-location-and-strips-1920`: No terminal strip yet. Place a terminal (I, terminal) or accept a proposal.
- 11 words, `70-set-location-and-strips-1920`: Select devices (click, or drag a box) to set their location
- 10 words, `16170-terminals-drawer-resize`: X 160.0 mm  Y 224.0 mm   Home zoom to fit
- 10 words, `16170-terminals-drawer-resize`: X 192.0 mm  Y 204.0 mm   Home zoom to fit
- 10 words, `70-set-location-and-strips-1920`: X -4.0 mm  Y 36.0 mm   Home zoom to fit
- 10 words, `70-set-location-and-strips-1920`: X 120.0 mm  Y 116.0 mm   Home zoom to fit
- 10 words, `70-set-location-and-strips-1920`: X 4.0 mm  Y 128.0 mm   Home zoom to fit
- 10 words, `7375-terminal-right-click-1920`: X 80.0 mm  Y 76.0 mm   Home zoom to fit
- 10 words, `7375-terminal-right-click-1920`: X 120.0 mm  Y 112.0 mm   Home zoom to fit

### Windows

- 12 words, `1104-windows-edges-1920`: Page, page group or cabinet, filed next to what you have open
- 10 words, `1100-navigator-moves-and-resizes`: X 108.0 mm  Y 64.0 mm   Home zoom to fit
- 10 words, `1104-windows-edges-1920`: X -72.0 mm  Y -4.0 mm   Home zoom to fit
- 10 words, `1104-windows-edges-1920`: X 728.0 mm  Y 464.0 mm   Home zoom to fit
- 10 words, `1104-windows-edges-1920`: X 676.0 mm  Y 464.0 mm   Home zoom to fit
- 10 words, `1104-windows-edges-1920`: X 140.0 mm  Y 64.0 mm   Home zoom to fit
- 10 words, `1104-windows-edges-1920`: X -160.0 mm  Y 64.0 mm   Home zoom to fit
- 10 words, `1104-windows-edges-1920`: X 16.0 mm  Y 44.0 mm   Home zoom to fit
- 10 words, `1121-dock-by-dragging`: X 44.0 mm  Y 76.0 mm   Home zoom to fit
- 10 words, `1121-dock-by-dragging`: X 104.0 mm  Y 156.0 mm   Home zoom to fit

### Workspaces

- 16 words, `02-workspaces`: Cable diagram · =A1-W1 · ÖLFLEX CLASSIC 110 4G1,5 · +CAB → +FLD · 12 m
- 15 words, `02-workspaces`: Wire list · 19 connections between device pins (supply and PE leads: see Wire production)
- 10 words, `02-workspaces`: Drag a device onto the plate · Ctrl+K to search
- 10 words, `630-ctrl-n-from-3d-shows-schematic`: Press Q to place a symbol · ? for keys
- 9 words, `02-workspaces`: Q or Space to place · Ctrl+K to search
