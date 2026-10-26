# Wire cut-and-label strips: handoff
Source: audit/creative/round36.md idea 2. Code: `novo_layout::workpack::add_strip_cards`, `CardKind::Strips`, `WireCut::ferrule`; PDF `strip_table` in `novo-pdf/src/workpack.rs`; ferrule text `ferrule_text` in `novo-app/src/app/workpack.rs`.
"Per phase" was read as per wire group (Power, Control, Outside the plate); the build order Phase enum has no electrical phases. Strips are outside the build order (NOT A STEP) and sit after the other cards. User finds it in the existing Work cards (PDF) command (3D > Layers > Build order, Bench panel, menu).
QA: scenario 500-workpack-cut-strips; pictures in qa/ws-cut-strips/.
