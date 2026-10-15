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
