# Wire test sheet: handoff
Branch claude/project-thread-gq30ww. Engine: crates/novo-commissioning/src/test_sheet.rs, store fields `insulation`, `test_settings` (serde default, old files read). UI: crates/novo-app/src/app/commissioning_test.rs, tab "Wire test" in Commissioning (workspace Commissioning, outline panel). PDF: "<project> test sheet.pdf" from the tab button.
Wiring order seam: `wiring_order()` = Build mode Wires-phase steps per cabinet (deduped), then remaining wires in walk order. Wire-it thread (not merged when written) should change the Build mode steps or this one function.
Misses: continuity miss -> set_check Failed -> Major punch item "Continuity failed"; insulation miss -> Blocking punch "Insulation low".
Resume: cargo test -p novo-commissioning --test test_sheet; cargo test -p novo-app --lib commissioning_test; scripts/qa.sh --only "Wire test".
Traps: new anchors must be in learn.rs + learn_labels.rs + `UPDATE_ANCHORS_DOC=1 cargo test -p novo-app anchors_doc`. CommissioningStore is no longer derive(Eq) (floats), manual impl.
