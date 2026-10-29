# Maker parts XML import

Where to find it: File > Import parts > Import parts from a CAD parts export (pick the
part file and its companion files together), the Parts window's Import catalog files, or a
folder in the parts inbox (`kopercad-cli parts import`, File > Import parts > Import parts from
a folder). Ctrl+K finds it with "maker xml", "data portal", "connection points" or "drilling".

## What it reads

Parts XML files of the kind makers publish for parts management programs. One file per part,
plus companion files, named `MAKER.ARTICLE.kind.xml`:

| Kind | What KoperCAD keeps |
| --- | --- |
| `part` | maker, order and type number, texts in every language (English shown, the others kept as `description_<lang>`), GTIN, eCl@ss, datasheet link, size, weight, rated current and voltage, power loss, conductor range, poles, main and auxiliary contact counts, coil voltage, frequency and type (the keys the device description recipes read) | A pole count such as `3+N` reads as 3; counts with a unit, ratings and currents are never read as contact counts or coil types.
| `connectionpoints` | per terminal: name, wire range (mm² and AWG), most wires, strip length, torque range, connection kind, position and direction in the maker's frame, other attributes as written |
| `construction` | holes: position, diameter, kind, repeat count and spacing (expanded with `DrillHole::positions`) |
| `accessorylist`, `accessoryplacementlist` | accessory part numbers with quantity and placement offset |
| `manufacturer`, `propertydefinition` | the maker's long name, names for property ids |

Reader: `novo-parts::dataportal` (`read_portal`). Units in values (`6,0 mm2`, `41,0 A`,
`0,5 Nm`) and decimal commas are read; the file's `length-unit` is applied.

## What is honest about it

The reader was written from format notes and tested on invented files. It has not seen a
maker's real file. So:

- Attributes are matched by what their names say, not by a fixed list. What it cannot place is
  kept as written (`eplan.<name>`, or the terminal's `extra`) and listed on the preview as
  "Kept as written, not understood", with a Copy format report button that shows names only.
- Positions of connection points and holes are kept in the maker's own frame
  (`maker_position_mm`, `DrillHole`). The corner they are measured from is not known, so
  `position_mm`, which wire routing and the 3D view trust, is left empty. Confirming the frame
  against a real file is the next step; the drill plan (Cabinet > Output) is a separate item.
- Connection data is marked `sources["connection_data"] = Estimated` until the reader has met a real maker file, so the wire checks only warn.
- The side a terminal faces is not worked out from its direction.
- Reading over a part already in the catalog keeps its price, ETIM class, lifecycle and files.
- Nothing is read from EPLAN macros, libraries or Data Portal pictures, and no maker data ships
  with KoperCAD; imported parts are marked `import:partsxml` and stay out of shared bundles.
