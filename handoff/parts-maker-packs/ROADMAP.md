# Parts: maker packs (lane b of "Parts library at scale")

Scope: one import recipe per maker (Wago, Phoenix Contact, Weidmüller, Siemens, Schneider, ABB, Rittal, Murrelektronik, Lapp), user-run only (open the page, the user downloads, we import), golden tests. Lane lead: thread "Parts library at scale" (owns catalog schema v2, #512). Never automate a login, never fetch by ourselves, never copy EPLAN's library.

Done (PR on #512's branch)
- `novo-parts::recipes`: route + steps, order-number checks, download recognition, mapping engine (BMEcat/CSV features to schema v2), nine recipes.
- Import reads CSV tables and BMEcat inside catalog ZIPs; Downloads watch knows each maker's number variants and file types.
- Parts window shows the maker's steps, an order-number warning, and what the recipe read.

Next, in order
1. Replace synthetic samples with real, redistributable maker files as the user supplies them (see `crates/novo-parts/tests/golden/README.md`).
2. Weidmüller, Siemens, ABB, Rittal, Murrelektronik, Lapp: capture the real part-page address and download route once on a normal machine; set `Route.page` and `Verified`.
3. ETIM feature codes (EF numbers) as a second matching path next to names, once the ETIM model file is bundled.
4. Excel (.xlsx) read, so the "save as CSV" step goes away (needs a reader crate; ask the coordinator).
5. Terminal positions and sides from STEP geometry (lane c).

Open questions
- Which makers' real BMEcat files can the user supply first?
- Written answers from makers on automated fetch (design doc 2a); until then every recipe stays hand-off.
