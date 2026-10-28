# Parts acquisition (real maker parts and CAD files)

Goal: importing real parts and their STEP/DXF files from makers works, with honest account guidance and a check that a file belongs to the part. Design: `docs/PARTS-LIBRARY-DESIGN.md` (3d-2). Rules: KoperCAD never signs in, never asks for or keeps a password, never scrapes.

Lanes (one PR each, from main):
- P1 CAD file matching and "Add CAD files" (this PR): `novo-parts::cadmatch`, Parts window panel, Downloads judge, account note.
- P2 data side: BMEcat 1.2, BMEcat prices (PRODUCT_PRICE_DETAILS, PRICE_QUANTITY), markup text kept, ETIM codes through the official ETIM release file the user supplies (no codes from memory), checks that did not fire (KC-09).
- P3 library export/import (move a library to another PC); data pool/API (2BA, Schneider Product Catalog API) only after the owner decides.
Not mine: KC-03 STEP size (PR 1563), KC-05 profile folder and CLI prices (PR 1562).
Open: switch `identity_of` from `step_box` to `novo_interop::part3d::measure_step` once 1563 is merged (size then becomes exact evidence).
