# Bulk parts import (`import-parts.cmd`, `kopercad-cli parts`)

Gets real maker data into the parts catalog with one command. Design limits
from `PARTS-LIBRARY-DESIGN.md` hold: **the tool never contacts a maker site**.
You download the catalog files (your own login, the maker's terms); the tool
reads, checks and merges them. No admin rights; it writes only under
`%APPDATA%\KoperCAD` (the parts catalog, its `.bak`, `import-report.txt`, `inbox`).

## Run it

1. Double-click `import-parts.cmd` in the `KoperCAD-latest` folder. First run
   creates `%APPDATA%\KoperCAD\inbox\` with a folder per maker and opens it.
2. Save downloads into the maker's folder (BMEcat `.xml`, `.csv`, or a `.zip`
   holding BMEcat). A maker without a folder: make a folder named after it.
   Wholesaler files that name the maker in every row go in `_wholesaler`.
   Spreadsheets must be saved as CSV first.
3. Double-click `import-parts.cmd` again. Re-run after every new download.

`kopercad-cli parts guide` lists, per maker, the page to use, whether a login
is needed and where to save. `--dry-run` reads and reports without writing.
`--inbox DIR` and `--catalog FILE` override the locations (used by tests).

## In the app

The same import runs from the app, no command line needed:

- **File > Import parts > Import parts from a folder** (command `parts.import_folder`,
  also in the Ctrl+K palette as "import parts"): pick a folder of catalog files
  (the picker opens in the inbox); the result and the files that held no parts
  are listed afterwards.
- **File > Parts**, header buttons: *Import catalog files* (pick files),
  *Import inbox*, *Open inbox folder*, *Open library folder*, *Install ETIM
  release* (see below), *Maker guide* (what to download per maker) and
  *New part...* (add a part by hand to your own catalog). A catalog file dropped on the window with no part selected is
  imported as a catalog.

A spreadsheet or price list with your own column layout (`.xlsx`, or a CSV that is not a maker
file) goes through the table import instead, with a mapping step: `PARTS-TABLE-IMPORT.md`.

## BMEcat 1.2 and 2005, prices and ETIM codes

Both BMEcat versions are read. 1.2 names its elements `ARTICLE`, `SUPPLIER_AID`,
`ARTICLE_DETAILS`, `ARTICLE_FEATURES`, `ARTICLE_ORDER_DETAILS`,
`ARTICLE_PRICE_DETAILS`; 2005 calls them `PRODUCT`, `SUPPLIER_PID`, and so on.

**Prices.** The list price is read with its currency (the line's, else the
catalog header's), the quantity it is for and the date it is valid from:
`42.00 EUR per 100`, `1850.00 EUR per 1000 m`, `98.70 EUR`. The quantity comes
from `PRICE_QUANTITY` (order units) times `NO_CU_PER_OU` (content units per
order unit) when a content unit is given. Unit codes (UN/ECE Recommendation 20:
`C62` piece, `MTR` metre, `PR` pair, `PK` pack, ...) are shown as words; a code
that is not known is kept as written, so a price is never shown for the wrong
unit. With several price lines the list price (`net_list`) with the smallest
quantity wins; a customer price (`net_customer`) is used only when it is the
only one, and the report says so. A price with no currency, a zero or negative
amount, a text that is not a number, a price factor that is not a positive
number, a quantity that is not a whole number, or an order unit that differs
from the content unit without `NO_CU_PER_OU` (a drum of metres: how many?) is
left out and listed under "parts to check", never guessed. A price written
with one separator and three digits after it (`1,500`, `1.500`) is read as
1.5, as BMEcat means it, and the report says so, in case 1500 was meant.

**Text.** Markup inside a description (`<b>`, `<br/>`, escaped `&lt;b&gt;`) is
removed and its words are kept.

**ETIM codes.** Makers write a feature as a code (`EF000008`, its unit
`EU570448`, a choice `EV000207`). What a code means is in the ETIM release from
ETIM International, which you download yourself (it is free; the download page
may ask you to sign in, in your own browser). KoperCAD carries no ETIM codes and
does not bundle the release. Install it once:

- in the app: **File > Parts > More > Install ETIM release (ZIP or CSV)** or
  **(folder)**, or
- `kopercad-cli parts etim <zip, folder or csv>`.

A compact copy is kept beside the catalog (`etim\`, follows the profile folder).
Every later import translates the codes it knows. The release's CSV tables are
recognised by the codes in their columns, not by file or column names, because
those differ between releases. A code the release does not know stays as it is,
and the report says how many codes were not translated and what to do.

**Not verified against a real file.** The BMEcat reader and the ETIM reader were
written from the public format descriptions and tested on made-up files in both
BMEcat versions and in the table layouts above. No real maker file or ETIM release
has been read yet. Put the first real files in `inbox` and run with `--dry-run`:
what the report lists under "did not map" or "not translated" is what to fix.

## What it does

Same readers and maker recipes as the one-click Import in the app
(`novo-parts::recipes`), over a whole folder:
BMEcat/CSV/ZIP -> maker recipe (SI values, connection, wire range, dimensions,
ETIM/ECLASS, price, lifecycle) -> checks -> merge by (maker, order number).
Same file twice changes nothing; a newer file updates; a `.bak` of the
catalog is kept. Checks (warnings, nothing is dropped): maker present,
description present, ETIM class `EC000000`, dimensions positive and under
3000 mm (also when a file gives only some of the three sizes), order number
looks like the maker's (the table import runs the same check), terminals vs
poles. An article with no order number is listed, not dropped silently.
The report lists per maker: parts, new, updated, same, without size, without
ETIM, without price, completeness (Good/Fair/Poor), unmapped features and parts to check.

## Limits

- Mappings were written from public wording and tested on synthetic samples
  (`crates/novo-parts/tests/golden/`); the first real file will show features the
  report lists under "did not map". Send that list to fix the recipe.
- Duplicate parts across files: the later file name (alphabetical) wins. When the two
  differ in price or description, "parts to check" names both files.
- Geometry (STEP/DXF) is not bulk imported; it stays one-click per part.
