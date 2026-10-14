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
- **Origin** is the insertion point. By convention it is the first connection
  point, usually the top-left one.
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
| `connections` | yes | Connection points (may be empty only for pure graphics). |
| `anchors` | no | Text placement points; a `tag` anchor is required unless `behavior.kind = "none"`. |
| `graphics` | yes | Artwork, drawn in order. |
| `behavior` | yes | Simulation behavior slot (a table, written last). |

Unknown fields are rejected, so typos fail loudly.

## Connections

`{ id, at, dir, role? }`

- `id`: default designation printed at the point, unique within the symbol.
- `at`: `[x, y]` on the 4 mm grid. A line or polyline of the artwork must
  start, end or bend exactly there.
- `dir`: `up`, `down`, `left` or `right`: the direction a wire leaves the
  point. Nothing in the symbol lies beyond the point in that direction.
- `role` (optional): `pe` for protective earth, `n` for neutral.

## Anchors

`{ role, at, align }` places a text that belongs to the instance, not the
artwork. The text is vertically centred on `at`; `align` is `left`, `center`
or `right` (default `left`). Roles: `tag` (device tag, such as `-K1`),
`function_text`, `address` (PLC address) and `part`.

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

## Function types

The function type says what the device does, independent of its artwork. It
drives cross references (a coil's contacts are shown under it), design checks
and the choice of simulation model. Lookups match prefixes, so
`Library::by_function("contact.no")` also finds `contact.no.push`.

| Prefix | Used for |
| --- | --- |
| `contact.no`, `contact.nc`, `contact.co` | Auxiliary contacts; suffixes name the actuator (`push`, `turn`, `estop`, `position`, `thermal`, `proximity`, `pressure`, `float`, `temperature`, `delay_on`, `delay_off`) |
| `contact.main` | Main (power) contacts of a contactor |
| `coil.relay`, `coil.timer.*`, `coil.valve` | Operating coils |
| `protection.*` | `fuse`, `mcb`, `mpcb`, `rcd`, `rcbo`, `overload` |
| `switch.*` | `disconnector`, `switch_disconnector`, `main` |
| `motor.*` | `ac3`, `ac1`, `dc` |
| `transformer.*`, `converter.*` | Transformers, power supplies, drives |
| `signal.*` | `lamp`, `horn`, `buzzer`, `bell` |
| `terminal`, `terminal.*`, `connector.*` | Terminals, plugs, sockets |
| `earth`, `earth.*` | Earth, protective earth, chassis |
| `connectivity.*` | Interruption points, potential feeds, terminal bridges |
| `passive.*`, `load.*`, `source.*`, `meter.*` | Components, loads, sources, instruments |
| `plc.input.*`, `plc.output.*`, `plc.supply` | PLC channels and supply |

## Behavior slot

`[behavior]` has a required `kind`; every other key is a parameter kept as
data, so the simulation workstream (WS-19/20) can add parameters without a
format change. The kinds and parameters below are provisional and owned by
simulation.

- `poles`: lists of connection ids that form one switched or conducting
  path. Two ids per path; a changeover lists `[common, break, make]`.
  The validator checks every id exists.
- `kind = "conductor"`: the `poles` are always joined inside the symbol
  (terminal sides, connectors, bridges). `Symbol::bridges()` returns them as
  connection indices, matching `SymbolInstance::bridges` in `novo-core`.
- `kind = "contact"`: `normally = "open" | "closed" | "changeover"`,
  optional `latching = true` (stays where put: selector, E-stop),
  optional `delay = "on" | "off"` (timer contact).
- `kind = "coil"`: optional `delay = "on" | "off"` (timer relay).
- `kind = "breaker"`: `trip = ["thermal", "magnetic", "residual"]`
  subset, optional `adjustable = true`.
- `kind = "motor"`: `phases = 0 | 1 | 3` (0 for DC).
- `kind = "converter"`: `input`, `output` (`ac` or `dc`).
- `kind = "meter"`: `quantity`.
- `kind = "passive"`, `"load"`, `"source"`: `type`.
- Other kinds: `none` (pure graphics such as earth), `conductor`
  (terminals, connectors), `fuse`, `overload`, `lamp`, `signal`,
  `transformer`, `plc_input`, `plc_output` (with `signal = "digital" |
  "analog"`).

The full list of accepted kinds is `novo_symbols::BEHAVIOR_KINDS`.

## Using a symbol in the model

`novo-core` stores a placed symbol's library key (`SymbolKey`) plus a copy of
its pins. Map a `Symbol` to it as: key = `id`, pins = `connections` in order
(name = `id`, offset = `at`), bridges = `Symbol::bridges()`. Only
`conductor` poles are bridges; contact, fuse and breaker poles are switched
and belong to simulation state. A future "refresh from library" must match
pins by designation, not index, so reordering a file never rewires existing
instances.

## Adding or changing a symbol

1. Add or edit a `.toml` file under `symbols/<standard>/<category>/`.
2. Run `cargo test -p novo-symbols`: every file is parsed and validated.
3. Look at it: `cargo run -p novo-symbols --example gallery -- gallery.html`
   writes a page with every symbol, its connection points and tag anchor.

Never rename or delete an `id` that has shipped; projects refer to it.
