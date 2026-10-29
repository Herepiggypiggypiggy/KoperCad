# Standard circuit library

KoperCAD ships its own library of generic circuits, written as plain TOML
under `crates/novo-parts/circuits/<n>-<chapter>/<name>.toml` and compiled at
run time into ordinary `.kcmacro` macros. Nothing is copied from another
product: every circuit is built from general electrical knowledge and the
IEC 60617 symbols KoperCAD already has. Licence: MIT.

## Where the user finds it

- **Place macro** window (Draw > Place macro, or Ctrl K and type "macro"):
  the list has a heading per chapter under `Circuits`, and a collapsible
  **Mounting view** under the preview.
- **Macros** browser: same folders, and the mounting view under the picture.
- Command line: `kopercad-cli macros install-circuits --lib <folder>`.

The first time either window opens, the circuits are put in the user's macro
folder next to the starters. Existing files are never overwritten, so a
circuit the user edited stays as they left it.

## Chapters

| Folder | Chapter | Circuits |
| --- | --- | --- |
| `Circuits/1 Supply and distribution` | supply | 24 V control supply, control transformer |
| `Circuits/2 Protection and starters` | protection | direct-on-line feeder, reversing feeder |
| `Circuits/3 Hoists` | hoists | single speed with spring brake, two speed (pole change) with rectifier-fed brake, frequency controlled with brake and braking resistor, brake rectifier with DC-side switching |
| `Circuits/4 Travel drives` | travel | two speed with brake, frequency controlled with brake |
| `Circuits/5 Safety` | safety | emergency stop with main contactor, emergency stop with safety relay and feedback |
| `Circuits/6 Operator stations` | control | radio receiver relays, pendant station with two-step buttons |
| `Circuits/7 Sensors and signalling` | peripherals | horn and beacon, limit switch permits, motor thermistor relay, cabinet fan with thermostat, anti-condensation heater with hygrostat |

## File format

One circuit per file. Unknown fields are refused. Keys: `name`, `chapter`,
`description`, `tags`, `notes`, `parameter`, `device`, `symbol`, `junction`,
`wire`, `chain`, `bus`, `open`, `plate`, `rail`. A device carries a `mount`
(kind `din`, `plate`, `door` or `field`, size, position, rail or place).

A file is checked when it is built, and the tests fail on:

- a pin that is neither wired nor listed in `open`;
- an unknown symbol, pin, chapter or device;
- a device without a symbol, or without a mounting entry;
- a device off the plate, off its rail, or on top of another device.

Placed circuits are also run through `novo-checks`: no wires on each other,
no contact drawn twice, no short between potentials, and a second copy gets
new tags.

## What it is and is not

- The circuits are **wiring examples**. They are not validated safety
  designs: category, performance level and fuse or cable sizes depend on the
  parts and the machine.
- Mounting sizes are typical sizes of generic devices. They size the plate
  and rails; the real size comes from the part the user assigns.
- Pins of drive control terminals use generic names; check them against the
  chosen drive.

## Adding a circuit

1. Add the TOML file in the chapter folder.
2. Add it to `SOURCES` in `crates/novo-parts/src/macros/circuits/mod.rs`
   (a test fails when the list and the folder differ).
3. `cargo run -p novo-parts --example circuit_preview -- <file.toml>` draws it
   as SVG.
4. `cargo test -p novo-parts --test circuit_library` and
   `cargo test -p novo-tests --test circuit_library_clean`.
