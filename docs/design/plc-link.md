# PLC engineering link (brief 9, lane A)

Extends `docs/design/plc.md` §6 ("a later interop step writes the same rows as a
PLC tag table ... reading that file back updates signal texts only"). Code:
`novo-plc` module `link`.

## What the programmer gets

The project already knows each channel's card, slot, address, signal and where
it is wired. The link writes that as the file the PLC programming tool
imports, so nobody retypes an I/O list:

| File | Menu / command | For | Addresses | Status |
| --- | --- | --- | --- | --- |
| I/O list CSV | File > Export PLC, `export plc` | Excel, any tool, and the way back | project style | tested end to end |
| PLCopen XML (TC6 v2.01) | `export plcopen` | CODESYS and tools that import PLCopen | `%IX4.3`, `%IW64` | written from the public schema, read back in tests; **not yet imported into a real tool** |
| Tag table `.xlsx` | `export tia` | TIA Portal tag tables | `%I4.3`, `%IW64` | column layout of a TIA tag table export; **not yet imported into TIA Portal** |
| Tag CSV | `export studio 5000` | Studio 5000 / Logix Designer | alias tags to `Local:3:I.Data.3`, `Local:5:O.Ch1Data` | published CSV layout; **not yet imported into Studio 5000** |
| Address plan CSV | `export address plan` | hardware engineer | per card: slot, byte ranges, drawn and spare | tested |

Also `kopercad-cli export plc <file> --format csv|plcopen|tia|studio5000|hardware -o <dir>`.
One file per station for the three tool formats (stations are couplers, plc.md §11);
files go next to the project file (`Plant PLC.xml`, `Plant PLC B.xml`) and only a file
this export wrote is replaced.

The first real import into each tool is the open validation step. Until someone
does it, the tool says nothing stronger than the table above.

## Tag names

One name per channel, unique over the whole export, valid as an IEC 61131
identifier in every target: ASCII letters, digits and single underscores,
starting with a letter, not a reserved word (`if` becomes `if_x`), at most 128
characters (40 for Studio 5000). Accents fold (`één` gives `een`).

- Source: the channel symbol's `plc.tag` if set, else the signal text
  (`Feed pump 1 running` gives `Feed_pump_1_running`), else `K4_DI3`.
- Duplicates get `_2`, `_3` (case-insensitively), inside the length limit.
- Names someone chose (`plc.tag`) are placed first, so a derived name yields to them.
- Spares are left out unless asked for; then `Spare_K4_DI3`.
- A channel with no address (card without a start, Rockwell card without a slot)
  is left out and named in the warnings; the CSV keeps it.
- Data types: `BOOL` for DI/DQ/RO, `INT` for AI/AQ.

## Reading it back

File > Import PLC tags reads any of the files above, finds the format from the
content, and opens a preview. Nothing changes until Apply (Enter).

- Matching: our CSV by card tag, type and channel number (survives an address
  style change); other files by address, in any style (`%IX4.3`, `I4.3`,
  `Local:3:I.Data.3`). An address that fits channels in two stations is not
  guessed: the preview says to use the CSV.
- What is written: the comment becomes the channel's `plc.signal`; a tag name that
  differs from what the export would have given becomes `plc.tag`. One undo step.
- **Addresses never come back.** They are derived from the cards. A file whose
  address differs from the project's is listed under "Addresses are never changed
  by an import" and left alone.
- Also listed, not applied: tags that match no channel, tags on spare channels
  (nothing drawn to hold a signal), a word tag on a bit channel and the reverse,
  the same channel twice (first counts).

## Model vs exact

Tag names, addresses and the address plan are exact (derived from the project).
The three tool files are written from public descriptions of the formats and
checked here only by reading them back with our own reader.

## Not done in this lane

- Hardware configuration import (GSD / EDS), fieldbus topology and diagnostics
  addresses: need device description files we do not have.
- Importing from a real CODESYS / TIA / Studio 5000 project: the programmer's
  extra tags (memory bits, DBs) are ignored by design; only I/O addresses match.
- Lane B (ISO 13849 safety functions) is a separate thread.
