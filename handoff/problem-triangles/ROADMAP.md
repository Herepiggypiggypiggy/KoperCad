# Problem triangles: roadmap

Scope: every problem or result in the program becomes a yellow or red triangle at the object that causes it (Tadjiro, 2026-11-04). Design: `docs/design/problem-triangles.md`. Audit: `LEDGER.md`, `sites/*.md`.

Done
- Phase 1: audit of 76 check kinds, 50 finding types, 776 sites, 14 channels (LEDGER.md).
- Phase 2: shared mechanism and lane plan (design doc, sections 3 to 12).

Next (order)
1. PR with these docs merged by the integration lead.
2. Coordinator adds the empty `novo-flags` crate (own PR), then starts lane T0a/T0b.
3. After T0b merges: lanes T1..T14 in parallel from main, then T15.

Open questions: design doc section 14.
