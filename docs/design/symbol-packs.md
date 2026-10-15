# Symbol packs and the NFPA pack

WS-25 (standards packs), built on `novo-symbols` (WS-05). This is the design
for shipping more than one drawing standard; it changes no UI and no
`novo-core` API.

## Goal

A US or Canadian customer draws to NFPA 79 / ANSI Y32.2 / IEEE 315 (the JIC
style: bar contacts, circle coils, circle pilot lights). A European customer
draws to IEC 60617. Both draw the same electrical functions, so everything
built on a function (cross references, potentials, checks, reports,
simulation) must work the same on either, with no `if nfpa` anywhere.

Non-goals: translating drawings between standards in bulk (a later command
can build on the equivalence below), pneumatic and hydraulic symbols, and a
standard-specific title block (that is a page template, not a symbol).

## Decisions

### 1. A pack is a directory under `symbols/`

`symbols/<pack>/<category>/<name>.toml`, id `<pack>/<category>/<name>`.
Packs are `iec` (today) and `nfpa` (new). The `standard` field names the
standard text (`IEC 60617`, `NFPA 79 / ANSI Y32.2`), `category` stays the
directory name, and the format stays at version 1: no field is added.

### 2. The same path means the same device

`nfpa/contacts/no` and `iec/contacts/no` are the same device drawn two ways.
A symbol exists in both packs under the same path below the pack root, or in
only one when the standard has no such device. Because of that, switching a
project's standard is a key rewrite: `Pack::counterpart(id)` swaps the first
segment and the instance keeps its pins.

To keep that safe, a symbol and its counterpart agree on everything the model
and the engines read, and only differ in artwork:

- `function` and `behavior` (kind, `normally`, poles, ports, parameters);
- connection ids, order, `role` and `optional` (EN 50005 / IEC 61666
  designations stay the defaults in both packs; an instance can renumber them,
  and a later setting can offer NFPA-style `L1/T1` designations);
- the tag anchor role, and all connection points on the 4 mm grid.

Connection positions may differ, since NFPA artwork has other proportions.
Refresh-from-library already matches pins by designation, not position.

`tests/packs.rs` enforces the agreement for every pair, so a pack cannot drift.

### 3. One pack is active, aliases are per pack

`Library::builtin()` stays the IEC pack, so nothing that exists changes
(about 50 call sites). New:

- `enum Pack { Iec, Nfpa }` with `Pack::ALL`, `id()`, `standard()`,
  `of(symbol_id)` and `counterpart(symbol_id)`;
- `Library::builtin_pack(Pack)`: the symbols of one pack;
- `build.rs` emits one table per pack, found from the first path segment.

Aliases and favourites are unique per pack, not across packs, so `no`, `coil`
and `mcb3` work in both and the palette needs no prefix. A catalog is built
from one library (`Catalog::new(Library::builtin_pack(pack))`), chosen by the
project's standard. Layering a user folder over it works as before.

The project setting that picks the pack (and its default per region) is a
WS-01/WS-02 change and is outside this work. Until it exists the NFPA pack is
reachable from code, Python and the symbol gallery only.

### 4. Drawing conventions

Same rules as the IEC set (vertical current paths, supply at the top, mm,
y down, 4 mm connection grid), with the NFPA look:

| Device | NFPA drawing |
| --- | --- |
| Contact NO / NC | Two bars across the conductor, 4 mm apart; NC adds a diagonal slash |
| Changeover | Break and make contact pairs sharing the common lead |
| Coil (relay, contactor, timer, solenoid) | Circle on the conductor, 16 mm between points |
| Pilot light | Circle with the colour letter (R, G, A, W, B); no X |
| Pushbutton | Contact bars plus a stem and bar (flat head); E-stop has a round head |
| Overload | Zigzag heater per pole; the contact is the NC bars with a thermal mark |
| Fuse | S-curve between the leads |
| Circuit breaker | Blade contact with an arc over it; thermal and magnetic marks as in IEC |
| Motor | Circle with `M`; no `3~` text |
| Transformer | Two coil strings with two core lines between |
| Terminal, earth, connectivity | Shared with IEC, drawn the same |

Where NFPA and IEC are identical (terminals, bridges, interruption points,
earth, PLC channels, plug connectors, supplies, passives), the NFPA pack
carries a copy under its own id instead of falling back to IEC, so a pack is
complete on its own and `builtin_pack(Nfpa)` never needs the other pack.

### 5. How the files are made

The IEC TOMLs are hand-maintained now (the old generator no longer
reproduces them). The NFPA pack is written by a standalone generator,
`handoff/ws-25-symbols/generator/gen_nfpa.py`, so the geometry helpers stay in
one place; the TOMLs are still the source of truth and may be edited by hand
once a symbol is final.

## Plan, small PRs

1. This document.
2. Pack plumbing (`Pack`, per-pack tables, `builtin_pack`), the NFPA core set
   (contacts, coils, contactors, operators, signalling, overload, fuse,
   breaker, motor, transformer, terminals, earth) and `tests/packs.rs`.
3. NFPA breadth: sensors, switches, measuring, sources, PLC, remaining
   protection, passives.
4. IEC library growth for both packs: more sensors, signalling, actuators,
   heating and cooling, solid-state relays, based on gaps found while
   drawing real starters and control panels.
5. Gallery switch for packs and a symbol-count table in `FORMAT.md`.

## Risks

- **Counting**: tests elsewhere that count or iterate `builtin()` are
  unaffected because `builtin()` stays IEC. Anything that switches to
  `builtin_pack` must be reviewed by its owner.
- **Looks wrong**: bar-contact art is easy to get subtly wrong. Each PR
  includes a rendered gallery image and Tadjiro's hands-on check is still the
  last word on appearance.
- **Alias overlap** across packs is deliberate; a combined library would
  report `DuplicateAlias`, so none is offered.
