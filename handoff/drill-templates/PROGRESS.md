2026-10-21 20:35 UTC cut-out cards, print-scale check page, command and window row built; tests and clippy green; QA 39 passes at both sizes. PR next.
2026-10-21 20:50 UTC review fixes: Good tested before Stretched (+ float epsilon, 100.2/99.8 test), accuracy wording now says 0.5 mm only if square is 100 mm else up to 0.2 %.
2026-10-21 21:10 UTC review round 2: printed-at box and absolute advice (second round lands on 100), ruler note, card note.
2026-10-21 21:05 UTC PR #1268 merged at main 3cbc2659 (reviews: Good before Stretched, accuracy wording, printed-at box). Lane done; next steps are in ROADMAP.md.
2026-10-22 00:50 UTC QA harness: keep_file verb (exports copied to files/ beside the report), any path component may hold one *; scenarios 39-42 keep the print scale check, workshop pack, cut-out cards, plate drawing PDF/DXF; fixture qa/fixtures/door-cutouts.novo (example qa_fixture_from_json).
