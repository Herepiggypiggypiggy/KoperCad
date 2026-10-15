# Parts library at scale: roadmap

Scope: gap #1 of docs/AMBITION-REVIEW.md (PR #504), brief 1. This thread owns lane (a): catalog schema, scale, import pipeline. Lanes (b) maker packs, (c) part quality, (d) geometry and pin positions run in their own threads and build on the schema.

## Done
- Schema v2 in novo-parts (`schema.rs`): classification, terminals, price, lifecycle, alternatives, assets, provenance. v1 files load unchanged.
- Indexed catalog (hash index, pre-lowered search docs); 100,000 parts search under 100 ms in release.

## Next
1. Parts window shows the v2 fields (class, price, lifecycle, terminals, provenance) and filters on them.
2. ETIM model file bundled with attribution; ETIM features to `data` keys.
3. Pipeline: BMEcat features and prices into v2, EDZ members, streaming import with progress, preview before write.
4. Large catalog storage (compact JSON above 5,000 parts, or SQLite) if load time needs it.

## Open questions
- Which makers does the workshop use most (decides pack order for lane b).

## Import pipeline plan (not started; paused at Tadjiro's request, 2026-10-15)
1. ETIM model: bundle the open-licence model file with attribution and licence notice (design section 3); class and feature names from it, codes stay in `CatalogPart.class`.
2. BMEcat: map features, ETIM/ECLASS class, prices (`Price`, per, date) and lifecycle into v2; `provenance` `import:bmecat`; one real public sample per maker as golden test.
3. EDZ: user-supplied zip; XML members to `novo-interop` reader (#347), terminals into `PartTerminal`; verify layout on a real file first.
4. Streaming import for 100,000+ parts: parse in a worker with progress, preview (added/changed/unchanged) before writing, `Catalog::retain` for replaced sets.
5. Large catalog file: compact JSON above 5,000 parts or SQLite; measure open time at 100,000.
6. Missing-data report hooks for the quality lane; ETIM attribution screen in the Parts window.
