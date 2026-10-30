# Controller in the live circuit (lane C2)

Plan: `plans/controller-live-view.md` (project files). Engine: `crates/novo-controller` (profiles, controls, signals,
interlocks, instances, bindings). This lane connects an instance of that engine to the drawing: a virtual radio, pendant
or joystick whose signals close the contacts of a receiver drawn in the schematic, so the logic of the circuit reacts
as on the crane. It simulates wiring and logic only: no radio protocol, no receiver firmware, no safety function
(not PL, SIL or compliance), and the shipped profiles are generic stand-ins, never a vendor product.

## Where the user finds it

- **Shift+L** (live view), then **Controller…** on the Live potentials card; or **Simulate > Controller panel**
  (command `sim.controller`, also in the command field: "controller", "radio", "pendant", "hoist").
- The Controller card sits on the right of the canvas. It shows one control per profile control (rockers as buttons that
  spring back when the pointer lets go, latches, E-stop with its own Reset, deadman and stop as hold buttons), the
  link, power and battery switches, the signal list with the receiver pin each one is bound to, the binding table
  and the checks. **Keyboard** turns the keyboard into the radio (keys are listed next to each control); it is off
  until asked so the canvas keys stay free.
- **Signal filter.** Above the signals and the binding table: a controller picker ("This controller", "All controllers" or
  one by name), a search box (all words must occur in the controller, the signal or its target), **Active** (only signals
  that are high) and **Unbound** (only signals that drive nothing yet), and Clear. With "All controllers" each line names
  its controller, so you can bind or check every instance from one list. The filter is kept while the program runs and
  is not saved in the project. The rules live in `novo_controller::monitor` (tested without a window).
- The fixture is `qa/fixtures/radio-hoist.novo` (a generic radio, a receiver, two reversing and two speed
  contactors, a two-speed motor). QA scenarios `1581` (1366x768) and `1581b` (1920x1080) drive it; `1582` opens it from the menu and the command field; `1583` runs it together with Simulate > Hoist run (the motor carries a 100 kg hook load, so the radio lifts the hook through the drawn circuit).

## The panel widgets (lane C3)

The card draws the controller the way the device looks, from a plan derived from the profile
(`novo_controller::panel::plan`), so a profile from the import wizard gets the right widgets with no code per controller:

| Control | Widget |
| --- | --- |
| Two-step or three-step rocker (`rocker2`, `rocker3`) | A stack of step zones per direction. Press the top zone for step 1 (slow), slide down or press the lower zone for step 2 (fast); a closed lower contact is tinted. Springs back on release |
| Joystick axes (`axis`, or rockers in a `joystick` profile) | A pad with a dot per detent. Two axes share a pad (name the pad with `stick = "..."` on a control, or the axes pair in order: first left-right, second up-down). Drag the knob to a detent; it springs back |
| `selector` | A rotary with a pointer: click turns to the next position, or click a position name |
| `estop` | The mushroom, latched; Reset twists it free |
| `latch`, `key` | A switch |
| `button`, `deadman` | A hold button; stop, horn and enabling bar get their own icon |

Next to the Keyboard switch: **Sticky** (a click keeps a button or the stick held until clicked again, so several motions run at once) and **Key map**
(a table of every key and what it does). The template follows the kind of controller: radio and pendant put safety and power first, then motions;
a joystick puts the pads first; a PLC panel or custom profile is one list.

The import wizard now also recognises a dead-man or enabling bar (`deadman` control), the tare button, a coupling switch and a fast-step release relay
(English and Dutch words); the release relay becomes a latch `fast_release` and every step-2 signal gets `requires = ["fast_release=on"]`. Hoist-unit selection is a
`selector`; two E-stop channels are two parallel contacts of the one mushroom.
### Several controllers at once

- The card starts with one **tab per controller instance**: a dot in the instance colour (ringed in green while a
  control is held away from rest, an E-stop pressed included) and its name. **+ Add** makes another instance of any
  profile; **Duplicate** makes one of the same kind with no bindings; **Name, colour and group** renames it (the id
  stays, so bindings and scripts keep working), picks a colour and names what it drives ("runway1/bridge2").
- **Pop out** gives an instance a floating panel of its own (a tab with the sign ⧉ shows it is out). Any number of
  panels can be open and operated at the same time, also with the card closed; closing a panel docks the instance back.
  They are panels inside the program window, not separate operating-system windows (not built yet). The keyboard drives
  one instance at a time: **Keyboard here** in a panel (or the selected tab) moves it.
- **Shared targets** (card and panel): when another instance also drives a target of this one, a line says who, who is
  pressing now, and whether it is a conflict (not marked `shared`, in the Check list as an error) or both are pressing.
  Engine side: `BindingTable::ownership`, `ControllerSet::is_active`.
- Instances, names, colours and groups are stored in `controllers.project` like the bindings. Which panels are popped out is
  not saved. Moving that property into the generic project block stays with the persistence lane.

## How a signal reaches the drawing

A binding connects a signal (`radio-1/HOIST_UP_FAST`) to a target, and each target kind has one meaning:

| Target | What it does while the signal is high |
| --- | --- |
| Pin of a device (`-A1:2`) | The switched pole that pin belongs to closes (a receiver's dry output, or any contact no coil drives). The pin must lead to exactly one pole: a shared common (`COM`) is refused with the reason. |
| Hand device (`-S1`) | Its poles go to the opposite of their rest state, as if the button were held. `invert` gives the opposite sense, as the E-stop chain needs (pressed while the signal is low). |
| PLC input (`%IX0.3`) | The bit is held for the running PLC program; the drawn channel is overridden. Bit inputs only; a word input is kept and reported by the checks. |
| Machine module path | Kept in the table; read by the machine loop (lane L5b). |

Receiver symbols (`iec/controllers/output_dry`, `outputs_6/9/12/16`) declare their shared common with
`common = "COM"` in `[behavior]` (validator and `FORMAT.md` know it): each output is one pole `[COM, n]`. The device
letter for them is A (IEC 81346 class A).

Per frame the live view calls `ControllerLive::drive` before it solves, so everything that solves sees the
controller (lessons and tests too). A pole that was bound last frame and is not now goes back to its rest state.
The hoist run (lane L5a) steps the circuit through `LiveView::solved` too, so the controller composes with it with no
extra code: its clock advances by each fixed tick, the radio closes the receiver contacts, the drawn contactors settle,
and the loop reads the motor feed. The pole forcing (`novo_sim::pins`) is the same `PoleState` mechanism the loop's
`set_contact` uses.

## Storage

One text property of the project, `controllers.project` (TOML of `ProjectControllers` with the profile texts it
uses), written in one undo step ("Controllers"). No change to `novo-core`. Controls are stored at rest: a project
saved with the E-stop pressed reopens with it released. A profile that changed or is missing is restored with a note;
data that cannot be read is kept untouched until the person discards it on the card.

## Checks

The engine's binding checks plus what only the drawing can say: a target that leads nowhere, two targets that close
the same contact, a coil-driven contact the controller cannot close, a PLC word input. Signals that drive nothing
are one line per controller, not one per signal. A target that mixes an `and` signal (an E-stop chain) with an
ordinary one gets a warning: while the ordinary signal is high it overrides an open chain, so a pressed E-stop would
not drop the target. Bind the chain to a target of its own. Not yet in the Check menu (`novo-checks`); the card shows them.

## Honest limits

- The receiver is modelled by the drawing, not by the profile: whatever the schematic does with the closed contact is
  what happens. If the real receiver latches, drops or delays, only what the profile's `requires`/`drops_when`/link
  rules state is modelled.
- Interlocks in the example (K1/K2, K3/K4) are the drawing's own; the engine refuses only what the profile says.
- Not a safety function: the E-stop chain shows the logic of the wiring.
- Generic profiles only. A person builds a vendor profile with File > Import > Controller profile from I/O table.
- The dead-man bar, tare, coupling switch and fast-step release are modelled from the drawing and the profile you enter. They are not a certified
  enabling or safety function (no PL, SIL or compliance claim), and the import wizard only recognises them by the wording of the table.

## Gamepad and joystick (lane C5)

Where the user finds it: Controller card (Simulate > Controller panel), section **Gamepad** under Key map. It shows the
device and whether it is connected, a live monitor of the sticks, triggers and buttons (with the step ticks of the mapped
axes), the dead-man state, what the pad holds (or, with driving off, what it *would* hold: the test), a short event log,
and the buttons **Edit map...** and **Calibrate...**. The switch "Drive this controller with the pad" is off until asked,
so plugging a pad in never moves anything.

Pieces:

- `novo_controller::gamepad` (pure, wasm clean, tested by hand in `tests/gamepad.rs`): `InputState` (axes and buttons by
  stable names such as `left_y`, `south`, `left_trigger`, plus numbered `axis3`/`button7` for odd devices), `GamepadMap`
  (TOML, saved per controller instance in the project as `[gamepad.<instance>]` of the controllers block), `PadMapper`
  (state in, holds and one-shot actions out), `Calibrator` (centre, then range, corners).
- `novo-app` `input_devices.rs`: the generic host input service. `InputBackend` is the trait; `FakeBackend` is what the
  tests and `kopercad --control` use (`pad` control command, QA step `pad axis left_y 0.8`). `input_gilrs.rs` reads real
  pads on Windows through `gilrs` (Windows Gaming Input: part of Windows, offline, no driver, no admin rights).
- `controller_pad.rs`: per-frame step, the section, the map editor and the calibration wizard.

Rules of the map (each has a test):

- A stick position picks the step of a rocker or axis. The deadzone is rest; the throw beyond it is cut into one band
  per step of that side (two bands for a two-step rocker, three for a three-step). A step is left again only when the
  value has fallen back by the hysteresis. The first direction is the positive one unless its name says left, down or
  back. A selector takes the band of the stick, or Next/Prev buttons.
- Calibration: per axis the raw centre and both ends, scaled per side; invert swaps the directions.
- Dead-man: with a dead-man input in the map, axes and hold buttons marked "dead-man" (the default) act only while it is
  held; letting go releases them in the same frame; after it is grasped again, a deflected stick (or a held button) must
  first return to rest. The E-stop button acts on the press edge whatever the dead-man says.
- Connecting: whatever is held when the pad connects must be let go first, and a button already down fires nothing.
- Losing the device (unplugged, or the system stops reporting it): every control is released at once, then the map's
  policy applies once: nothing more, drop the radio link (it comes back with the device; the main latch stays dropped), or
  press the E-stop (never reset by reconnecting).
- The pad acts only while the card is open, the window has the focus and the controller is the selected one. A window in
  the background counts as hands off (not as unplugged). During calibration the controller sees a pad at rest.
- The pad holds controls exactly like the mouse and keys do (`HoldBy::Pad`, which wins over key and mouse while it asks
  for something); interlocks, the E-stop latch, `requires` and the link rules stay the engine's.

Honest limits: it simulates the wiring and logic; it says nothing about a real transmitter, its radio, latency or contact
bounce, and neither the dead-man gate nor the E-stop button is a certified safety function. Real pad reading is built
only for Windows; elsewhere the section says so (`gilrs` needs the system library `libudev` on Linux, which the CI image
lacks), and mouse, touch and keyboard still work. Devices without a standard layout (no SDL mapping) connect but show
nothing. Real hardware was not available to the author: the Windows reader compiles for the Windows target but has not
been run against a physical pad.

## Add-on seam

The whole feature is two files (`controller_live.rs`, `controller_card.rs`), one `mod` line in `app.rs`, one field
(`ctl`) in `LiveView`, a handful of hook calls in `live.rs` and `plc_run.rs` (held input overrides), the command
`sim.controller` and the lesson anchor `live.controller`. When the add-on manager hosts it, those are the places to
move behind the add-on switch.
