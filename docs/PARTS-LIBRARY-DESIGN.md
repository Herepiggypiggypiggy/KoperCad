# Parts library: Wago, Phoenix Contact, Schneider, Siemens

Status: proposal (research and design only, no code). Owner of the crate: WS-07 (`novo-parts`).
Written 2026-10-15 from the project thread "Can we build an EPLAN-style parts library?".

## 1. The short answer

We can build a parts library that does what a designer needs (pick a real
article, get its ratings, size, terminals and symbol), but not by copying
EPLAN's library, and not by bundling the makers' CAD files into the
installer. The workable model is:

1. A **small curated core** we author ourselves (facts only), shipped with the app.
2. **Per-user import** of the makers' own free data, fetched by the user on
   demand and stored in their profile.
3. Everything works **offline** once imported, and nothing needs admin rights.

EPLAN's library is not EPLAN's own: the Eplan Data Portal is an exchange
platform where manufacturers supply the data. So the data we want exists
at the makers, in neutral formats. The portal and the EDZ packaging are
EPLAN's, and we do not use them as a source.

## 2. What is verified and what is not

Research tools were web search and page fetch only. No maker terms-of-use
page was read (the fetches returned the product pages, whose footers only
link to the legal text). **So every redistribution question below is open
until someone reads the actual licence text, or asks the maker.**

### Verified from a source (page read 2026-10-15)

| Fact | Source |
| --- | --- |
| Eplan Data Portal data is supplied and updated by the manufacturers (commercial data, function templates, logic macros, 2D/3D graphics, connection points, drilling patterns, DXF). | eplan.com/en/support/data-portal.html |
| Eplan-format download needs an Eplan software service agreement; a free Eplan Cloud account gives access to the portal under the Eplan Cloud terms; DXF is a choice. | same |
| WAGO offers CAE data for EPLAN, WSCAD, Zuken, StriePlan, AutoCAD Electrical, SEE Electrical; classification as eCl@ss and ETIM; 2D/3D CAD in 58 formats via WAGO PARTcommunity (CADENAS); the page calls the CAE data free. | wago.com/global/.../engineering-data |
| WAGO page does not state a login requirement, API or licence terms; DXF is not mentioned. | same (absence) |
| Phoenix Contact supplies product data for EPLAN (P8 file generator, Eplan Data Portal), WSCAD, SEE, AutoCAD Electrical, Zuken, AmpereSoft and others; it mentions ECLASS and BMEcat. | phoenixcontact.com/en-gb/product-data-for-your-engineering-process |
| That page does not mention STEP, DXF, ETIM, an API, a login or terms. | same (absence) |
| Phoenix Contact distributes product data to customers in ECLASS BMEcat format and maps its CAE data to ECLASS from 7.0. | eclass.eu best-practice entry |
| Schneider Electric has per-product CAD downloads ("Main Documents", CAD button, format picker). The FAQ does not list the formats or mention EPLAN, and states no terms. | se.com FAQ FA372855 |
| ETIM Classification Model is licensed under the Open Data Commons Attribution Licence ("free to use for everyone"); redistribution needs attribution and the licence notice kept. Language versions may be limited to national members. | etim-international.com/?p=1811 |

### From a secondary source only

- Siemens delivers CAx data (EDZ for EPLAN) through the Industry Online
  Support "CAx data" tab and a CAx Download Manager that needs Java. Source is a
  third-party blog, not Siemens. The same blog says plain CAD downloads hold
  only 2D/3D graphics without connection points.

### Inferred, not verified (treat as hypotheses)

- STEP is available for all four makers (very likely; not confirmed per maker above).
- Siemens and Schneider provide ETIM and ECLASS values; ECLASS Advanced
  content may require a paid ECLASS licence, ECLASS Basic is generally free. Check eclass.eu.
- Makers' terms probably allow personal use of downloads in a design project
  but not redistribution of the files as a library. This is the common
  pattern, and is exactly why we assume no redistribution until proven otherwise.
- Bare technical facts (rated current, width) are not copyrightable the way
  files and descriptive text are, but a large systematic copy of a maker's
  database can still be restricted (database rights in the EU). Take legal advice before shipping any bulk set.
- An EDZ file is a zip with XML part data plus macro files. We have not
  opened one; the exact layout must be checked on a real file before writing a parser.

## 3. What we may ship versus fetch per user

| Item | Ship in the installer | Fetch per user |
| --- | --- | --- |
| Our own authored part rows (order number, maker, type, a few rated values we typed from datasheets, our own descriptions) | Yes, small set (section 6) | n/a |
| ETIM class and feature codes | Yes, with attribution and licence notice | Language versions that are member-only: fetch |
| Our own symbols (IEC 60617, drawn by us in `novo-symbols`) | Yes | n/a |
| Maker STEP / DXF / 2D symbol files | **No** | Yes, user downloads from the maker, we import from a file or folder |
| Maker EDZ / EPLAN macros | **No**, and not parsed by default | Optional: user-supplied file, read for facts only (open question, section 9) |
| Maker BMEcat / ECLASS catalogs | **No** | Yes, user-supplied file |
| Maker descriptions, images, datasheet PDFs | **No** | Link only |
| EPLAN's library or Data Portal content | **No, never** | **No** |

"Fetch per user" means the user obtains the file themselves (browser or
the maker's own tool) and points KoperCAD at it, or later, if a maker
documents a public API with terms that allow it, KoperCAD fetches it with
the user's own account. We never proxy through a server of ours, and
nothing is needed beyond a normal user profile.

## 4. Offline and no admin rights

- The catalog is a file under `%LOCALAPPDATA%\KoperCAD\Catalog\` (already so in
  `novo-parts`: `user_catalog_path`). Imported maker files are copied into
  `...\Catalog\Sources\<maker>\` so a part keeps working when the download is gone.
- A project stores its own copy of every part it uses (already true:
  `novo_core::Part`), so a `.novo` file is complete without the catalog.
  Geometry used by the drawing (symbol, outline) is copied into the project too.
- No network is needed to open, edit or print. Fetch/import is an explicit user
  action and fails soft. No installer step, service or HKLM entry is involved.
- A company folder (any readable path, e.g. a share) can be added as an extra
  catalog source, same as the block libraries in DESIGN.md section 7.

## 5. Proposed part data schema

The current `CatalogPart` (maker, order number, type, description, letter
codes, keywords, dimensions, free `data`) stays valid. Version 2 adds
structure so classification, terminals and graphics are first-class.
Existing files load unchanged (`format: 1` is read, `2` is written).

```jsonc
{
  "format": 2,
  "parts": [{
    "manufacturer": "WAGO",
    "order_number": "2002-1201",
    "type_number": "2002-1201",
    "description": "2-conductor through terminal block, 2.5 mm2",   // ours, not copied
    "letter_codes": ["X"],
    "keywords": ["terminal", "feed-through"],
    "dimensions": { "width_mm": 5.2, "height_mm": 0, "depth_mm": 0 },
    "data": { "rated_current_a": 24, "rated_voltage_v": 800 },
    "class": { "etim": "EC000897", "eclass": null },              // codes only
    "terminals": [                                                // connection points of the part
      { "name": "1", "kind": "screwless", "wire_mm2": [0.08, 4.0], "side": "top" },
      { "name": "2", "kind": "screwless", "wire_mm2": [0.08, 4.0], "side": "bottom" }
    ],
    "function": "terminal",                                        // maps to a symbol in novo-symbols
    "assets": [                                                    // optional, local, never shipped
      { "kind": "step", "path": "Sources/wago/2002-1201.stp", "sha256": "..." },
      { "kind": "dxf2d", "path": "Sources/wago/2002-1201.dxf", "sha256": "..." }
    ],
    "provenance": { "source": "authored|import:bmecat|import:csv|import:step",
                    "file": "wago_2024.xml", "imported": "2026-10-15",
                    "licence_note": "user-supplied; not redistributable" }
  }]
}
```

Rules:
- `provenance` is required on every part. Anything with `import:` is excluded
  from export bundles and from the installer by construction.
- `class.*` holds codes, not text. ETIM text comes from the ETIM model file.
- `terminals` and `function` are what unlocks real value: a terminal strip
  can check wire range, a symbol can be chosen from the part, and cabinet
  layout gets the outline from `dimensions` or the STEP bounding box.
- This is a `novo-parts` format; if a field needs to live in the project
  `Part`, WS-02 decides. Today it fits in `Part.properties`.

## 6. Import path

One importer trait per source, all producing `CatalogPart` plus a report:

1. **CSV/Excel** (exists): `import_catalog` with header mapping. First tool, works with
   any maker price list or the user's own sheet.
2. **BMEcat / ECLASS XML**: Phoenix Contact names this as its format. Read
   article, features, ETIM/ECLASS class. Needs one real sample file to
   write the mapping from, we have none yet.
3. **STEP**: read only the bounding box (width, height, depth) and keep the
   file as an asset. No B-rep processing in Phase 1; a 3D viewer is later.
4. **DXF 2D**: keep as an asset; later extract an outline for the layout view.
5. **EDZ** (optional, user-supplied): read part facts and connection points
   only if the sample shows it is workable, and only if the terms allow it.
   We do not copy EPLAN macros or symbols into KoperCAD.

Every import shows a preview (what is added, what changes, what is skipped
and why) before writing, the same as `preview_merge` today.

## 7. First set of parts per maker (authored by us, from datasheets)

Small on purpose: enough for Gate 1 style drawings and for tests. Order
numbers must be checked against the maker's current catalogue by whoever
types them (the existing starter set already carries that warning).

| Maker | Family | Why |
| --- | --- | --- |
| WAGO | 2002 series feed-through terminals (2.5 and 4 mm2), PE terminal, 2002 end plate and jumper | Typical strip content |
| WAGO | 221 and 2273 splicing connectors | Common in panels, no strip |
| Phoenix Contact | PT 2,5 / PT 4 push-in feed-through, PT-PE, end cover, bridge | Same role as WAGO, lets the join rule be tested across makers |
| Phoenix Contact | UT 2,5 / UT 4 screw terminals, UT PE | Screw variant |
| Siemens | 3RT2 contactors S00 to S3, 3RV2 motor protection breakers, 5SL MCBs, 3RU thermal relays | Already partly in the starter file |
| Siemens | S7-1200 CPU and a few SM modules | PLC channel lists (novo-plc) |
| Schneider | LC1D contactors, GV2 motor breakers, iC60 MCBs | Same roles as Siemens |
| Schneider | ATV320 / ATV340 drives, M241/M262 CPU | Motor drive and PLC cases |

Target: roughly 15 to 25 rows per maker. We type facts from the datasheet.
We do not paste maker descriptions.

## 8. Phased plan

| Phase | Deliverable | Needs |
| --- | --- | --- |
| P0 | Read the four makers' terms (and ECLASS licence). Ask each maker in writing whether a user-side import and a small authored subset are acceptable. Record answers in this doc. | A person; nothing to build |
| P1 | Schema v2 in `novo-parts` (read v1, write v2), `provenance`, `terminals`, `class` fields. Authored sets for the 4 makers (section 7). Attribution screen for ETIM. | WS-07 |
| P2 | BMEcat importer and STEP bounding-box reader, with preview. User points at a file or folder. | One real BMEcat and a few STEP files per maker |
| P3 | Asset store, outline-from-STEP for cabinet layout, symbol choice by `function`. | WS-15, WS-05 |
| P4 | Optional EDZ reader, optional maker web fetch where terms allow, company-folder catalogs, 3D view. | P0 answers |

## 9. Open questions for the owner

1. May we spend time on P0 (contacting makers) or should the legal read be done by someone else?
2. Is reading an EDZ file the user already owns acceptable to him, given EPLAN's terms? Default: skip until P4.
3. Which Siemens and Schneider PLC and drive families does he actually use? That fixes the section 7 list.
