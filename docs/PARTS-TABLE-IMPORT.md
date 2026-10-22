# Parts table import (CSV and Excel with a mapping step)

For a parts table that is not a BMEcat file and not one of the maker layouts the
bulk import knows: a price list, a wholesaler export, a colleague's spreadsheet,
datasheet values typed into Excel. You say what each column is, check what every
row becomes, then add the parts. Nothing is added before that last click.

## Where to find it

- **File > Import parts > Import parts from a table (CSV or Excel)**, or Ctrl+K
  and type "table" (command `parts.import_table`).
- **File > Parts**, header button *Import table...*; an `.xlsx` file dropped on the
  Parts window opens it too.

## The steps

1. **Pick the file.** CSV, TSV, TXT (UTF-8 or the Windows-1252 that German and
   Dutch Excel writes, delimiter `;`, `,` or tab found by voting over the first
   lines, so a title line does not confuse it) or `.xlsx` (several sheets: pick
   one). Old `.xls` and `.ods` are refused with a plain sentence: save as `.xlsx`
   or CSV.
2. **Columns.** Each column gets a suggestion with the reason (English, German and
   Dutch header words, a unit in the header such as `[A]`, `(mm)`, `Verlustleistung in W`,
   ETIM and ECLASS codes recognised by their shape). Change any of them in the
   dropdown. Columns that are not recognised are kept as technical data under their
   own name, or ignore them. The header row is found automatically (title lines above
   the table are skipped) and can be changed.
3. **Rows.** Every row with its result (new, replaces an entry, already there,
   not imported) and every finding with the column it is about. Counts per field
   show what the file does not say.
4. **Add.** The parts are merged into your catalog (same maker and order number
   replaces, a second import of the same file changes nothing). The mapping is
   remembered for files with the same headers (`parts-table-mappings.json` next to
   the catalog) and a report is written next to the catalog
   (`parts-table-import-report.txt`).

## What is converted and checked

| Field | Reads | Stored as |
| --- | --- | --- |
| Rated current, AC-1/AC-3 current, setting range | `9`, `500 mA`, `1.2 kA` | A |
| Voltage, coil voltage | `24`, `230 V`, `0.4 kV` | V |
| Power loss | `1,2`, `800 mW`, `1000 BTU/h` | W |
| Rated power | `4`, `4000 W`, `5 hp`, `10 PS` (metric hp, Dutch `pk`) | kW |
| Width, height, depth | `45`, `6 cm`, `2 in`, `0.5 ft` | mm |
| Conductor range or min/max | `0.14 ... 4 mm²` | mm² |
| Price | `12,50 €`, `1.234,56 EUR` | amount, currency |
| Price list date | `2026-10-15`, `15.10.2026`, `15-10-2026`, `10/2026`, an Excel date | date on the price (the day of the import when no column, or an unreadable cell, says so) |
| Price is per (pieces) | `100`, `per 100`, `1000 Stk.` | quantity the price is for; a price per metre, kg or set is left out, not shown as a piece price |
| ETIM, ECLASS | `EC000066`, `27-14-11-01`, `27141101` | codes |
| Lifecycle, connection method | words in EN, DE, NL | enum |

A unit inside a cell beats the column's unit. A cell with no unit anywhere is taken
in the stored unit and the Rows step says so. Decimal commas, thousands separators
(`1.234,56`) and Excel's power-of-ten order numbers (`1.23E+11`, digits lost) are
handled or flagged.

Never guessed: a value that is not a number, has a unit that does not fit, is not
possible (a 45 m wide relay, a negative loss) or is an invalid class code is left
out of the part and the finding says so. `1,500` is ambiguous (1.5 or 1500): it is
read as 1.5 and flagged. A date written `05-10-2026` is read day first (as German and
Dutch lists write it) and the row says so when month first would be a different date;
`10/2026` is the first of that month. A row with no order number or no maker (no
default maker set) is not imported.

## The same part in several rows

The catalog holds one entry per maker and order number, so two rows with the same
order number cannot both be kept. The Rows step lists them (count, rows, which row is
used) and offers the choice: **Last row** (the default, as an update list reads),
**First row**, or **First row, gaps filled from the others**. Every row that is not the
one used says so in its findings, and the footer counts the repeated order numbers.

## Header words

Headers are matched without accents and case (`Höhe`, `Anschlüsse`, `Gültig ab`), in
English, German and Dutch: for example `Bedrijfsstroom`, `Warmteverlies`,
`Aansluitwijze`, `Aantal klemmen`, `Prijsdatum`, `Prijseenheid`, `Leverstatus`,
`Preiseinheit`, `Gültig ab`, `Valid from`. A header that is not known is shown as
"Other data" and the person chooses. `AWG` sizes are not converted (not a linear unit);
the cell is left out with a finding.

## Honest labelling

- **This file is** (Maker data / Typed by a person / Estimates / Not stated) is
  recorded as the source of every imported value. *Not stated* is the default: the
  value then shows "source not stated" in calculations, never "maker".
- Every part gets provenance `import:table`, the file name and the date, and is
  marked not redistributable.
- A field no column provides stays unknown. The Rows step shows how many parts have
  each field (dimensions, power loss, rated current, conductor range, terminals,
  class, price, lifecycle), and Parts quality scores the rest as it always did.
- Terminals are created only for a part whose function column says `terminal` and
  whose file gives the number of connections (1 to 8), numbered 1 to N like the
  maker readers do.

## Limits

- Excel: `.xlsx` only, values as the sheet shows them (formulas read as their last
  saved result). Merged cells and hidden sheets are read as they are stored.
- One part per row. A table with a row per terminal or per variant is not folded.
- Units outside the list above are refused, not guessed. Tell us which maker uses one.
- ETIM/BMEcat exports and datasheet PDFs keep their own paths (inbox import and
  the datasheet-to-part flow); the same completeness scoring applies to all.
