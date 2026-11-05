# Roadmap: EMC and mechanical on the enclosure spec

Lane E5 (EMC and mechanical) of `handoff/cabinet-start/CHAIN.md`. Reads `novo_layout::spec::EnclosureSpec` (E1, #1891).

1. Done: door, depth and plate checks judge against the chosen enclosure (`novo_layout::spec_fit`, `novo-checks` door checks); missing numbers and a typed-size mismatch are problems.
2. Done: EMC-22 lowest cavity resonance of the chosen box.
3. Next, needs fields the record does not have yet: wall material, seams, gland plates and cable entries (shielding indices), weight and load rating (weight against the plate/enclosure rating), maker door type. Ask the cabinet-start thread to add them to the record with provenance, then extend EMC-22 and the mechanical report.
4. Next: the door dialog (`cabinet_door.rs`) shows the record's numbers and marks overrides `typed` (owned by the Door and 3D lane).
