# PARTS-Q parts quality: roadmap

Scope (spawn table lane PARTS-Q): completeness score per part, missing-data report, duplicate and conflicting-value finder across sources, review queue that marks unreviewed imported parts untrusted. Pure engine (`novo-parts/src/quality.rs`) plus a Quality tab in the Parts window (`novo-app/src/app/parts_quality.rs`). Never invent data. Do not edit catalog.rs or schema.rs (Parts library lane).

1. PR 1: score, report, CSV, Quality tab. MERGED (#568).
2. PR 2: duplicate groups and conflict finder (Duplicates tab). In review.
3. PR 3: review queue, Review tab. In review.
4. Later: untrusted badge in the Parts list (needs a second hook line in parts.rs, ask the Parts library lane), CLI `kopercad-cli parts quality`.

Open: where the Parts list should show the untrusted badge.
