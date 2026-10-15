# PLC and I/O (WS-17)

How NovoElectric handles PLC cards, their channels, I/O addresses, the I/O
list and channel cross references. This extends `docs/DESIGN.md`; where the
two disagree, DESIGN.md wins and this file is corrected. Engine code lives in
`novo-plc`; the pieces other crates draw or print are listed in §8.

## 1. What the designer thinks

"Feed pump 3 running goes to the PLC" (DESIGN §1). The designer thinks in
**signals**: a contact or sensor whose state the program reads, or a coil or
valve it drives. The card, the channel number and the address are
bookkeeping. They matter on the printed set and to the PLC programmer, and
they must never disagree between the overview, the schematic and the list.

What goes wrong in page-first tools (EPLAN):
- The address is typed on the channel symbol, on the PLC overview and in the
  PLC navigator, and the three drift apart.
- Addresses are assigned by a separate "addressing" run that renumbers
  everything, so a card added in the middle moves addresses the PLC program
  already uses.
- A channel drawn twice, or a card with more channels drawn than it has, is
  found by the panel builder.
- The I/O list is exported, edited in Excel and never comes back.

## 2. The objects

| Thing | In the model | Stored | Derived |
| --- | --- | --- | --- |
| PLC station | A text name shared by its cards (`plc.station`) | the name | its cards in slot order |
| Card (CPU, I/O card, coupler) | A `Device` with `kind = Plc` | slot, start addresses, card layout override | address ranges, free channels |
| Card layout (what channels it has) | The part's `plc.channels` (`DI16`, `DI8 DQ8`, `AI4`), else the device's | in the part | channel list |
| Channel | A symbol of the card (`SymbolRole::Device { main: false }`) with `plc.channel` | the channel number | address, label, I/O list row |
| Card overview | The card's main symbol (`main: true`), optional | | its channel image |
| Signal text | The channel symbol's `plc.signal` | only when typed | otherwise the connected device's description |

A card is a device like any other: it has a tag (`-A4`, letter code A from
the shared IEC 81346-2 table, `plc.*` functions), a part, a location and
cross references. A channel is one function of that device, exactly as a
contact is one function of a contactor. That is the whole idea: **PLC I/O
reuses device ownership**, so references, renaming, Shift+G, the inspector
and the device list work on day one with no PLC-specific navigator.

The `plc.*` keys above are properties (`Properties`), so this needs no
`novo-core` API change. §9 lists what WS-02 may later turn into typed
fields.

### Card layout

`plc.channels` is a space-separated list of groups, read left to right:

| Group | Means | Address space | Width per channel |
| --- | --- | --- | --- |
| `DI16` | 16 digital inputs | inputs | 1 bit |
| `DQ8` (or `DO8`) | 8 digital outputs | outputs | 1 bit |
| `RO4` | 4 relay outputs | outputs | 1 bit |
| `AI4` | 4 analog inputs | inputs | 16 bits |
| `AQ2` (or `AO2`) | 2 analog outputs | outputs | 16 bits |

Channels are numbered from 0 per type in the order written (`DI8 DQ8`:
DI 0-7, DQ 0-7; `DI8 DQ8 DI8`: the second DI group is DI 8-15). Inside its
space a card packs digital channels bit by bit and starts each analog
channel on a word. The part supplies the layout; a device-level `plc.channels`
is an override (marked, resettable, DESIGN §7), for cards with no part yet.
A card with neither has no channels and the Issues list says so.

## 3. Addresses

An address names a channel for the PLC program: `%I4.3`, `%QW66`.

**Allocated like tags, then kept.** A card gets its start address in each
space it uses (`plc.start.i`, `plc.start.q`, in bytes) when it is added: the
next free byte range in its station, after the cards already there. From then
on the start is stored, exactly as a tag is: adding, moving or deleting
another card never changes it, because the PLC program already uses it.
This is the same reasoning as the pinned flow for pages (DESIGN §8).

**A channel's address is always derived:** card start plus channel offset.
It is never typed, never stored on the symbol and never drawn as a literal
text, so it cannot drift. Rename the card start in the inspector and every
channel label, the overview and the I/O list follow in the same undo step.

| Change | What happens |
| --- | --- |
| Add a card | Next free range in its station, one undo step with the add |
| Change a card's start (inspector) | Its channels follow; an overlap is refused while typing, with the card it would overlap |
| Change the part to a bigger card | Its range grows; if that overlaps the next card, a problem is marked on both (never a silent shift) |
| `renumber io` | Re-packs the station in slot order, with a preview listing every address that moves; one undo step. The only command that moves addresses |
| Delete a card | Its range becomes free; nothing else moves |

Digital and analog share a space as on most PLCs: a 16-channel DI card takes
2 bytes of the input space, a 4-channel AI card 8 bytes (aligned to a word).

**How addresses read is a rule**, one sentence in the Rules document:
"PLC addresses are written [IEC 61131 ▾ | Siemens | Rockwell | Plain]".

| Style | Digital input | Analog output |
| --- | --- | --- |
| IEC 61131 (default) | `%IX4.3` | `%QW66` |
| Siemens | `%I4.3` | `%QW66` |
| Rockwell | `Local:3:I.Data.3` (slot:channel) | `Local:5:O.Ch1Data` |
| Plain | `I4.3` | `QW66` |

Until WS-02 adds a typed rule, the project property `plc.address_style`
holds it. The style changes text only, never which channel is which.

## 4. Drawing channels

Channel symbols already exist in `novo-symbols` (`iec/plc/digital_input`,
`digital_output`, `relay_output`, `analog_input`, `analog_output`), each
with an `address` anchor.

- **Placing.** The grammar is mnemonic plus device, as for contacts
  (`no K1`): `di A4` places the next undrawn DI channel of `-A4`;
  `di` alone takes the card last used on this page, else the only card with
  a free DI channel, else asks in the surface by listing the cards with free
  DI channels (pickable rows, DESIGN §4). Sticky: the next click places the
  next free channel. Typed words: `di`, `dq`, `ai`, `aq`, `ro`, with Dutch
  aliases from WS-08's table (`ingang`, `uitgang`).
- **From a signal.** With a contact, sensor or coil selected, `to plc`
  (`naar plc`) places the next free channel of the right type next to it and
  connects it: an NO contact or sensor gets a DI, a coil or valve a DQ. One
  undo step. This is the "running signal to the PLC" sentence as a command.
- **Choosing a channel.** The inspector of a channel shows its card and
  channel as one chip (`-A4 · DI 3`). Picking another free channel moves the
  identity, not the drawing. A taken channel is listed with where it is
  drawn, not hidden.
- **Labels.** At the `address` anchor the canvas draws the derived address
  (`%IX4.3`); below the tag, the card location (`-A4 2.1`) as for contacts.
  These are scene labels like cross reference labels, not `Text` records.
  The signal text sits at `function_text` when the symbol has that anchor.
- **The overview.** The card's main symbol, when drawn, carries a channel
  image like a coil's contact image: one line per drawn channel,
  `DI 3  %IX4.3  3.4`, and a faint `spare` for undrawn ones. G on a channel
  jumps to the overview and back; Shift+G lists every channel of the card.
  Drawing the overview is optional. Without it, channels refer to the
  card's first drawn channel in reading order; unlike a contact without a
  coil, that is not a problem.

There is no PLC navigator and no "PLC box" object: the card is a device, the
drawer lists it, and the drawing shows it.

## 5. In the drawer

The Devices tab lists cards like every device. A card row expands to its
channels (DESIGN §3 keeps one drawer with three tabs; I/O is not a fourth):

| Address | Channel | Signal | Connected to | Drawn at |
| --- | --- | --- | --- | --- |
| `%IX4.0` | DI 0 | Feed pump 1 running | `-K1:13` | 3.4 |
| `%IX4.1` | DI 1 | (spare) | | |

- Signal texts are edited in the table (big edits are data edits, DESIGN
  §1); find and replace works as for terminals.
- Dragging a spare row onto the drawing places that channel.
- Selecting a row selects its symbol and the other way round.

## 6. The I/O list

A generated report (`io`), owned by the Outputs thread in `novo-reports`
and fed by `novo-plc`'s rows. One section per station, one row per channel
including spares, in address order:

| Column | From |
| --- | --- |
| Address | derived, in the project's style |
| Type | DI, DQ, RO, AI, AQ |
| Card | tag and slot (`-A4`, slot 3) |
| Channel | number among the card's channels of its type |
| Pin | the channel symbol's pin name, else the part's terminal designation |
| Signal | `plc.signal`, else the connected device's description |
| Connected to | the first device on the channel's net beyond terminals (`-K1:13`), and the field terminal (`-X2:7`) |
| Drawn at | `page.column`, or `spare` |

It prints as generated pages (DESIGN §8: its own section, `30a`, `30b`) and
exports to Excel with the other reports. A later interop step (WS-24) writes
the same rows as a PLC tag table for the PLC programming tool, one file per
station; the signal text becomes the tag comment. Reading that file back
updates signal texts only, never addresses (round trip without drift).

## 7. Problems

Marked in place and listed in Issues (WS-14 runs them through `novo-plc`):

| Problem | Severity | Marked on |
| --- | --- | --- |
| Two cards' address ranges overlap | Error | both cards (overview or first channel) |
| A channel is drawn twice | Error | both symbols |
| A channel number is beyond the card's layout | Error | the symbol |
| A channel symbol's type does not match its channel (DI symbol on a DQ) | Error | the symbol |
| A card has no layout (no part and no `plc.channels`) | Warning | the card |
| A drawn channel is connected to nothing | Warning | the symbol |
| Two cards in one station share a slot | Warning | both cards |

Spare channels are not a problem; they are a column in the I/O list.

## 8. Who builds what

| Piece | Crate | Thread |
| --- | --- | --- |
| Card layouts, address allocation and formatting, channel index, labels, I/O rows, problems, `renumber io` and `to plc` as commands | `novo-plc` | WS-17 |
| `di A4` placement and `to plc` in the command surface, aliases | `novo-workflow`, `novo-editor` | WS-08, WS-06 (calling `novo-plc`) |
| Address and card labels in the scene, channel image | `novo-canvas` via the xref label path | WS-03, WS-10 |
| G / Shift+G between channels and the overview | `novo-xref` | WS-10 (channels are ordinary device functions already) |
| I/O list report and its pages | `novo-reports` | Outputs |
| Problems in the Issues list | `novo-checks` | Outputs |
| Card rows expanding to channels in the Devices tab | `novo-app` | WS-01 |
| `plc.channels` on catalog parts | `novo-parts` | WS-07 |
| Typed fields and the address-style rule (§9) | `novo-core` | WS-02 |
| PLC tag table export (§6) | `novo-interop` | WS-24 |

`novo-plc` depends on `novo-core` only (plus `novo-symbols` for symbol
functions), so every crate above can call it.

## 9. Model asks for WS-02 (later, not blocking)

Version 1 uses properties so nothing waits on `novo-core`. Once the shape
has settled in use, WS-02 may make these typed:
- `ProjectRules::plc { address_style }` in place of `plc.address_style`;
- the card fields (`station`, `slot`, `start.i`, `start.q`, `channels`
  override) as `Device.plc: Option<PlcCard>`;
- `plc.channel` and `plc.signal` on channel symbols.

## 10. Engine pull requests, in order

1. This document.
2. `novo-plc` skeleton: card layout parser, address spaces, allocation of a
   new card's start, address formatting in the four styles. Pure functions,
   unit tested.
3. Channel index: which symbol draws which channel, labels, free channels,
   the "next channel" query placement needs; the problems of §7.
4. Commands: add card with allocated start, place next channel, `to plc`,
   `renumber io` with preview, all as `novo-core` commands (one undo step).
5. I/O list rows for the Outputs thread, and the rows for the Devices drawer.

## 11. Open questions

- Rockwell addressing is slot and channel, not bytes. Is the byte model plus
  a style enough, or does a Rockwell project need its own allocation? (v1:
  the style reads slot and channel; allocation is unused there.)
- Safety PLC channels (F-DI with two pins and a test pulse output) need a
  two-pin channel symbol and a pairing rule; deferred to a later PR.
- Distributed I/O (several couplers per station on a fieldbus): one station
  per coupler, or stations nested? v1: one station per coupler, named by
  the user.
