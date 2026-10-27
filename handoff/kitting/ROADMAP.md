# Kitting and stock optimiser: roadmap

Source: creative audit round 37, proposal 3 (`audit/creative/round37.md`).

Scope: before the first cut, pack every rail and duct cut into the stock lengths the shop buys (saw kerf, offcuts), total the wire per size and colour on reels, count ferrules, lugs, screws and labels, and give one pick list per build phase and one shopping list merged with the BOM.

Done (PR on branch `claude/project-thread-oblbwd`):
- `novo_reports::kitting::cutting`: exact 1D cutting-stock search (mixed bar lengths, kerf, held bars and offcuts first, splice of pieces longer than a bar). Tested against an exhaustive search.
- `KitSettings` (project property `kitting.settings`), `OnHand` text grammar.
- `kit_plan` and the `Kitting` report (id `kitting`): pick list per phase (Prepare, Mount, Wire, Label), saw plan per bar, reels, offcuts, shopping list, notes on exact versus planning values.
- UI: Reports > Bench output > Kitting tab, Reports > Export > Export kitting list (PDF), file `3 Kitting list.pdf` in the Workshop pack, Ctrl+K "kitting".
- QA scenarios 320, 321, 322.

Next, in order:
1. Per-wire reel assignment on the bench cut list ("cut from reel 2") once Wire-it lands.
2. Duct fill added by wires into the kitting notes (needs Wire-it duct accounting).
3. Separate packs per cabinet when the shop builds cabinets one at a time.
4. Supplier price per bar and reel, so "least cost" replaces "least length" (today cost is length).
5. Screw size by hole diameter and plate thickness (needs a fastener table from the person).

Open questions for Tadjiro (batched): real bar and reel lengths and pack sizes of the supplier; whether partial reels should be tracked across projects.
