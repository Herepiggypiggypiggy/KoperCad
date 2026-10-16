# PARTS-Q parts quality: roadmap

Scope (spawn table lane PARTS-Q): completeness score per part, missing-data report, duplicate and conflicting-value finder across sources, review queue that marks unreviewed imported parts untrusted. Pure engine (`novo-parts/src/quality.rs`) plus a Quality tab in the Parts window (`novo-app/src/app/parts_quality.rs`). Never invent data. Do not edit catalog.rs or schema.rs (Parts library lane).

1. PR 1: score (6 criteria, weights 20/20/15/15/15/15), report, missing-data CSV, Quality tab with filter, detail and export. DONE, in review.
2. PR 2: duplicate groups (same order number under another maker spelling, same type number under two order numbers) and conflict finder (two versions of one part, incoming merge vs catalog, internal inconsistencies); Duplicates sub-view.
3. PR 3: review queue. Sidecar `parts-review.json` next to the catalog (fingerprint per part, so a changed part needs review again). Imported (`import:*`) parts without a matching review are untrusted; authored parts are trusted. Approve and Reject in the Quality tab.
4. Later: untrusted badge in the Parts list (needs a second hook line in parts.rs, ask the Parts library lane), CLI `kopercad-cli parts quality`.

Open: where the Parts list should show the untrusted badge.
