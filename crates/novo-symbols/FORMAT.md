# Symbol format, version 1

Every schematic symbol is one TOML file under `crates/novo-symbols/symbols/`.
The files are the source of truth: they are embedded into the binary at build
time (`Library::builtin()`), and the same loader reads a folder of user or
company symbols (`Library::load_dir`). Nothing here depends on `novo-core`;
the data model refers to a symbol by its `id` and reads its connection points
and function type from this crate.

## Conventions

- **Units** are millimetres.
- **Axes:** x to the right, **y downwards** (screen and SVG convention).
- **Origin** is the insertion point and must lie on the grid. Usually it is
  the top-left connection point; where that would push artwork off-grid
  (changeover contacts, potential feeds, PLC outputs) it is another grid
  point near the symbol's top left.
- **Connection grid:** every connection point lies on a 4 mm grid
  (`novo_symbols::GRID`). Place symbols with their origin on the sheet's grid
  and wires meet connection points exactly.
- **Orientation:** symbols are drawn for vertical current paths, supply at the
  top, as in IEC-style schematics. The editor rotates and mirrors instances.
- **Sizes:** single contacts and coils are 16 mm between connection points;
  the poles of multi-pole devices are 8 mm apart; terminals are 8 mm.
- **Stroke width** is the renderer's choice; files contain geometry only.
- **Connection designations** follow EN 50005 / IEC 61666 defaults: coils
  `A1`/`A2`, make contacts `13`/`14`, break contacts `21`/`22`, overload
  auxiliaries `95`-`96` and `97`-`98`, main contacts `1`…`6`, motors
  `U1 V1 W1`, protective earth `PE`. An instance may renumber them.

## Example

```toml
format = 1
id = "iec/contacts/no"
name = "Make contact (NO)"
description = "Normally open contact, used for auxiliary contacts of relays and contactors."
standard = "IEC 60617"
category = "contacts"
function = "contact.no"
keywords = ["no", "make", "normally open", "auxiliary"]
aliases = ["no"]
favorite = true

connections = [
  { id = "13", at = [0, 0], dir = "up" },
  { id = "14", at = [0, 16], dir = "down" },
]

anchors = [
  { role = "tag", at = [-6, 8], align = "right" },
]

graphics = [
  { kind = "line", from = [0, 0], to = [0, 5] },
  { kind = "line", from = [0, 16], to = [0, 11] },
  { kind = "line", from = [0, 11], to = [-4, 4] },
]

[behavior]
kind = "contact"
normally = "open"
poles = [["13", "14"]]
```

## Top-level fields

| Field | Required | Meaning |
| --- | --- | --- |
| `format` | yes | Always `1`. |
| `id` | yes | Path below `symbols/` without `.toml`, e.g. `iec/contacts/no`. Lowercase `a-z 0-9 _ -` segments. Stable: projects store it. |
| `name` | yes | Display name. |
| `description` | no | One or two sentences for the symbol picker. |
| `standard` | yes | `IEC 60617` for this set. |
| `category` | yes | Must equal the directory the file is in. |
| `function` | yes | Function type, dotted from general to specific (see below). |
| `keywords` | no | Search words for the picker and command palette. |
| `aliases` | no | Short names the palette matches exactly, such as `no`, `coil` or `mcb3`. Up to 12 characters of `a-z 0-9 _ -`, unique within a library. |
| `favorite` | no | `true` to offer the symbol in the palette before anything is typed. |
| `connections` | yes | Connection points (may be empty only for pure graphics). |
| `anchors` | no | Text placement points; a `tag` anchor is required unless `behavior.kind = "none"`. |
| `bridges` | no | Groups of connection ids always joined inside the symbol, whatever its behavior (e.g. PE in and out of a drive). |
| `graphics` | yes | Artwork, drawn in order. |
| `behavior` | yes | Simulation behavior slot (a table, written last). |

Unknown fields are rejected, so typos fail loudly. `aliases`, `favorite`
and the contact parameter `center_off` were added to version 1 before its
first release; builds older than them reject files that use them. That includes unknown
keys in `[behavior]` (see below).

## Connections

`{ id, at, dir, role?, optional? }`

- `id`: default designation printed at the point, unique within the symbol.
- `at`: `[x, y]` on the 4 mm grid. A line or polyline of the artwork must
  start, end or bend exactly there.
- `dir`: `up`, `down`, `left` or `right`: the direction a wire leaves the
  point. Nothing in the symbol lies beyond the point in that direction.
- `role` (optional): `pe` for protective earth, `n` for neutral.
- `optional` (default `false`): a spare point that may stay unconnected,
  such as a second PE clamp. Design checks do not report it as open.

## Anchors

`{ role, at, align }` places a text that belongs to the instance, not the
artwork. The text is vertically centred on `at`; `align` is `left`, `center`
or `right` (default `left`). Roles: `tag` (device tag, such as `-K1`),
`function_text`, `address` (PLC address) and `part`.

Texts stay readable under rotation: when an instance is rotated, the editor
moves each anchor and each artwork text with the symbol but keeps the text
itself horizontal (or reading bottom-to-top for 90° and 270°), never upside
down.

## Graphics

Every primitive takes an optional `stroke = "dashed"` (default solid); closed
shapes take `fill = true` (default false).

| `kind` | Fields |
| --- | --- |
| `line` | `from`, `to` |
| `polyline` | `points` (2 or more), `closed`, `fill` |
| `rect` | `at` (top-left corner), `size` `[w, h]`, `fill` |
| `circle` | `center`, `r`, `fill` |
| `ellipse` | `center`, `rx`, `ry`, `fill` |
| `arc` | `center`, `r`, `start`, `end` in degrees, `end > start` |
| `text` | `at`, `text`, `size` (character height, default 2.5), `align` (default `center`) |

Arc angles: a point at angle `a` is `center + r * (cos a, sin a)`. Because y
points down, 90° is straight down and increasing angles run clockwise on
screen. `start = 180, end = 360` is the upper half of a circle.

## Quick insert

The command palette inserts common symbols without the picker:
`Library::by_alias("mcb3")` returns the symbol that lists `mcb3` in its
`aliases` (exact match after trimming and lowercasing, so `MCB3` works), and `Library::favorites()` returns the
symbols marked `favorite = true`, ordered by id. The built-in set has about
20 favourites (make and break contacts, coil, timer, contactor, MCBs, motor
protection, overload, fuse, motor, terminal, PE terminal, terminal bridge,
potential feed, push buttons, E-stop, lamp, PLC input) and about 90 aliases.
Loading fails with `LoadError::DuplicateAlias` when two symbols of one
library claim the same alias; when user and built-in libraries are combined,
the editor decides which wins.

The `favorite` flags are shipped defaults. A user's own favourites are
stored per user (under the user's config directory) and layered over them;
the editor never writes back into the library files, which may sit in a
read-only install folder.

## Terminals

Every terminal clamp is drawn the same way: a circle of radius 1.2 mm on the
conductor between two connection points, 8 mm apart per level (16 mm for
the fused and disconnect terminals, which have a clamp at each end). A terminal strip of 400
clamps of one type is 400 instances of one symbol, and the model stores only
the symbol key and pins per instance, so they cannot drift apart. Variants
(`terminal_n`, `terminal_pe`, multi-level, fused, disconnect) reuse the same
clamp and add marks around it. Jumpers between clamps are the
`connectivity/bridge_*` symbols; plug-in connector pieces are
`terminals/plug`, `socket` and `plug_socket`. A strip is closed with
`terminals/end_plate` and held on the rail by `terminals/end_bracket`;
both are pure graphics (`behavior.kind = "none"`) with no connections and
function types under `accessory.terminal`, so `by_function("terminal")`
returns only real clamps.

## Function types

The function type says what the device does, independent of its artwork. It
drives cross references (a coil's contacts are shown under it), design checks
and the choice of simulation model. Lookups match prefixes, so
`Library::by_function("contact.no")` also finds `contact.no.push`.

| Prefix | Used for |
| --- | --- |
| `contact.no`, `contact.nc`, `contact.co` | Auxiliary contacts; suffixes name the actuator (`push`, `turn`, `key`, `estop`, `position`, `thermal`, `proximity`, `pressure`, `float`, `temperature`, `delay_on`, `delay_off`) |
| `contact.main` | Main (power) contacts of a contactor |
| `coil.relay`, `coil.timer.*`, `coil.valve` | Operating coils |
| `protection.*` | `fuse`, `fuse_switch`, `mcb`, `mpcb`, `mcp`, `rcd`, `rcbo`, `overload`, `surge` |
| `switch.*` | `disconnector`, `switch_disconnector`, `main` |
| `motor.*` | `ac3`, `ac1`, `dc` |
| `transformer.*` | `1ph`, `3ph`, `current` |
| `converter.*` | `ac_dc`, `dc_dc`, `vfd`, `soft_starter` |
| `sensor.*` | `temperature.rtd`, `transmitter` |
| `signal.*` | `lamp`, `horn`, `buzzer`, `bell` |
| `terminal`, `terminal.*`, `connector.*` | Terminals, plugs, sockets |
| `earth`, `earth.*` | Earth, protective earth, chassis |
| `connectivity.*` | Interruption points, potential feeds, terminal bridges |
| `passive.*` | `resistor`, `capacitor`, `inductor`, `filter`, `diode`, `transistor`, `rectifier`, `potentiometer` and variants |
| `accessory.terminal.*` | Terminal strip accessories without connections: `end_plate`, `end_bracket` |
| `load.*`, `source.*`, `meter.*` | Loads, sources (including `source.generator`), instruments |
| `plc.input.*`, `plc.output.*`, `plc.supply` | PLC channels and supply |

## Behavior slot

`[behavior]` has a required `kind` plus parameters. The simulation workstream
(WS-19/20) owns their meaning and may add parameters; until then each kind
accepts exactly the keys below and the validator rejects anything else
(`novo_symbols::behavior_keys(kind)` returns the list).

Two parameters name connections, and they mean different things:

- `poles`: conducting **paths**. Each group lists the connections one path
  joins: two ids, `[common, break, make]` for a changeover, or two or more
  for a `conductor` such as a bridge bar. No id may
  appear in two groups. Whether a path conducts depends on the kind: always
  for `conductor`, by switch state for `contact`, `breaker`, `fuse` and
  `overload`, through the load for `coil`, `lamp`, `signal`, `meter`,
  `load`, `passive` and `source`.
- `ports`: groups of connections that together form one **winding or
  port** of a motor, transformer or converter, such as `["U1", "V1", "W1"]`
  or `["L+", "M"]`. Any size; no id in two groups.

| `kind` | Parameters (required in bold) |
| --- | --- |
| `none` | none (pure graphics: earth, interruption point, potential feed) |
| `conductor` | **`poles`** (always joined: terminals, connectors, bridges) |
| `contact` | **`poles`**, **`normally`** = `open` \| `closed` \| `changeover`, `latching` (bool; selector, E-stop), `delay` = `on` \| `off`, `center_off` (bool, changeover only: a middle position where the common joins neither side, as on an I-0-II switch) |
| `coil` | **`poles`**, `delay` = `on` \| `off` |
| `fuse` | **`poles`**, `switched` (bool) |
| `breaker` | **`poles`**, **`trip`** (non-empty subset of `thermal`, `magnetic`, `residual`), `adjustable` (bool) |
| `overload` | **`poles`** |
| `motor` | **`ports`**, **`phases`** = 0 (DC) \| 1 \| 3, `speeds` (integer) |
| `transformer` | **`ports`** |
| `converter` | **`ports`**, **`input`**, **`output`** = `ac` \| `dc` |
| `source` | `poles` or `ports`, `type`, `phases` |
| `load`, `lamp`, `signal`, `meter` | `poles`; `load` also `type`, `meter` also `quantity` |
| `passive` | `poles`, `type`, `wiper` (connection id of a potentiometer's wiper) |
| `plc_input`, `plc_output` | **`signal`** = `digital` \| `analog` |

## Bridges and pass-through

`Symbol::bridges()` returns the groups of connection indices that are always
joined inside the symbol: the `poles` of kinds that never open
(`ALWAYS_CLOSED`: `conductor` and `overload`, whose main circuit 1-2, 3-4,
5-6 is the heater elements in series) plus the top-level `bridges` field.
Use the field for permanent links inside devices whose behavior is something
else: series elements that always conduct (current transformer primary,
ammeter, line reactor, EMC filter lines) and links such as
`bridges = [["PE", "PE2"]]` on a drive, so a PE continuity check sees the
motor behind it as earthed.

`Symbol::pass_through()` adds the switched `poles` of contacts, breakers and
fuses (`PASS_THROUGH`). A potential (L1, N, 24V) and its naming carry through
these, so L1 stays L1 from the feed through fuse, breaker, contactor and
overload to the motor terminals. Loads (coils, lamps, motors, meters) end a
potential and are in neither list. Simulation decides whether a switched
path actually conducts. A changeover's `[common, break, make]` becomes two
paths, common-break and common-make, and a potential propagates outward from
the side it arrives on, so the break and make sides are never merged.

## Using a symbol in the model

`novo-core` stores a placed symbol's library key (`SymbolKey`) plus a copy of
its pins. Map a `Symbol` to it as: key = `id`, pins = `connections` in order
(name = `id`, offset = `at`), bridges = `Symbol::bridges()`; potential
propagation follows `Symbol::pass_through()`. Never store `pass_through()` in
`SymbolInstance::bridges`: it only propagates potential names and must not
take part in netlist connectivity or potential conflict checks (it returns a
`PassThrough` newtype so this can't happen by accident). A future "refresh from library" must match
pins by designation, not index, so reordering a file never rewires existing
instances.

## Adding or changing a symbol

1. Add or edit a `.toml` file under `symbols/<standard>/<category>/`.
2. Run `cargo test -p novo-symbols`: every file is parsed and validated.
   For a user or company library, `Library::load_dir_lenient` loads the
   valid symbols and returns the problems with the rest.
3. Look at it: `cargo run -p novo-symbols --example gallery -- gallery.html`
   writes a page with every symbol, its connection points and tag anchor.

Never rename or delete an `id` that has shipped; projects refer to it.
